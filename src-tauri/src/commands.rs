use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::{
    app_state::{AppState, CompanionSnapshot, TosuConnection, current_snapshot, update_snapshot},
    auth,
    tosu::process::{MemoryAccessError, TosuMemoryAccess, TosuProcess},
};

#[tauri::command]
pub async fn get_snapshot(app: AppHandle) -> CompanionSnapshot {
    current_snapshot(&app).await
}

#[tauri::command]
pub async fn set_tracking_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    set_tracking_enabled_impl(&app, enabled).await
}

pub async fn set_tracking_enabled_impl(app: &AppHandle, enabled: bool) -> Result<(), String> {
    app.state::<AppState>().tracking.send_replace(enabled);
    update_snapshot(app, |snapshot| {
        snapshot.tracking_enabled = enabled;
        snapshot.tosu.connection = if enabled {
            TosuConnection::Searching
        } else {
            TosuConnection::Disabled
        };
        snapshot.tosu.message = None;
        if !enabled {
            snapshot.osu.running = false;
            snapshot.osu.state = crate::app_state::OsuState::Unknown;
            snapshot.now_playing = None;
            snapshot.live_play = None;
        }
    })
    .await;
    Ok(())
}

#[tauri::command]
pub async fn set_tosu_auto_start(app: AppHandle, enabled: bool) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        state
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .set_tosu_auto_start(enabled)?;
    }
    update_snapshot(&app, |snapshot| {
        snapshot.tosu.auto_start = enabled;
    })
    .await;
    Ok(())
}

pub fn schedule_tosu_auto_start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let enabled = {
            let state = app.state::<AppState>();
            let settings = state
                .settings
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            settings.tosu_auto_start()
        };
        if !enabled || !current_snapshot(&app).await.tosu.executable_available {
            return;
        }

        for attempt in 0..4 {
            if app
                .state::<AppState>()
                .shutting_down
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            let (enabled, endpoint) = {
                let state = app.state::<AppState>();
                let settings = state
                    .settings
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                (settings.tosu_auto_start(), settings.endpoint())
            };
            if !enabled {
                return;
            }
            let reachable = tokio::time::timeout(
                Duration::from_millis(300),
                tokio::net::TcpStream::connect(endpoint.socket_addr()),
            )
            .await
            .is_ok_and(|result| result.is_ok());
            if reachable {
                return;
            }
            if attempt < 3 {
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
        }

        let snapshot = current_snapshot(&app).await;
        if snapshot.tracking_enabled
            && snapshot.tosu.executable_available
            && !snapshot.tosu.process_owned
            && !matches!(
                snapshot.tosu.connection,
                TosuConnection::Connected | TosuConnection::Connecting | TosuConnection::Stale
            )
        {
            let _ = launch_tosu_impl(&app).await;
        }
    });
}

#[tauri::command]
pub async fn launch_tosu(app: AppHandle) -> Result<(), String> {
    launch_tosu_impl(&app).await
}

pub async fn launch_tosu_impl(app: &AppHandle) -> Result<(), String> {
    let snapshot = current_snapshot(app).await;
    if !snapshot.tracking_enabled {
        return Err(
            "Resume tracking before launching tosu so Companion can detect an existing process"
                .into(),
        );
    }
    if matches!(
        snapshot.tosu.connection,
        TosuConnection::Connected | TosuConnection::Connecting | TosuConnection::Stale
    ) && !snapshot.tosu.process_owned
    {
        return Err("tosu is already running outside Companion".into());
    }

    let (result, memory_access) = {
        let state = app.state::<AppState>();
        let mut process = state
            .process
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let result = process.launch();
        (result, process.memory_access_status())
    };
    match result {
        Ok(_) => {
            update_snapshot(app, |snapshot| {
                snapshot.tosu.process_owned = true;
                snapshot.tosu.executable_available = true;
                snapshot.tosu.memory_access = memory_access;
                snapshot.tosu.connection = if snapshot.tracking_enabled {
                    TosuConnection::Searching
                } else {
                    TosuConnection::Disabled
                };
                snapshot.tosu.message = Some(if snapshot.tracking_enabled {
                    "tosu was launched; waiting for its v2 API".into()
                } else {
                    "tosu was launched; Companion tracking is paused".into()
                });
            })
            .await;
            Ok(())
        }
        Err(error) => {
            let message = match error {
                crate::tosu::process::ProcessError::NotInstalled => {
                    "tosu was not found at the selected path or on PATH".to_owned()
                }
                _ => error.to_string(),
            };
            update_snapshot(app, |snapshot| {
                snapshot.tosu.connection = TosuConnection::Error;
                snapshot.tosu.message = Some(message.clone());
            })
            .await;
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn stop_owned_tosu(app: AppHandle) -> Result<(), String> {
    stop_owned_tosu_impl(&app).await
}

pub async fn stop_owned_tosu_impl(app: &AppHandle) -> Result<(), String> {
    let result = {
        let state = app.state::<AppState>();
        let mut process = state
            .process
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        process.stop_owned()
    };
    match result {
        Ok(()) => {
            update_snapshot(app, |snapshot| {
                snapshot.tosu.process_owned = false;
                snapshot.tosu.connection = if snapshot.tracking_enabled {
                    TosuConnection::Searching
                } else {
                    TosuConnection::Disabled
                };
                snapshot.tosu.message = Some("Companion-owned tosu was stopped".into());
                snapshot.osu.running = false;
                snapshot.osu.state = crate::app_state::OsuState::Unknown;
                snapshot.now_playing = None;
                snapshot.live_play = None;
            })
            .await;
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub async fn grant_tosu_memory_access(app: AppHandle) -> Result<(), String> {
    let configured_path = {
        let state = app.state::<AppState>();
        let process = state
            .process
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        process.configured_path().map(ToOwned::to_owned)
    };
    let result =
        tokio::task::spawn_blocking(move || TosuProcess::grant_memory_access_for(configured_path))
            .await
            .map_err(|_| "the system authorization task could not be completed".to_owned())?;

    match result {
        Ok(_) => {
            let externally_running = {
                let snapshot = current_snapshot(&app).await;
                snapshot.tosu.connection == TosuConnection::Connected
                    && !snapshot.tosu.process_owned
            };
            let restart_result = {
                let state = app.state::<AppState>();
                let mut process = state
                    .process
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                if process.is_owned() {
                    process
                        .stop_owned()
                        .and_then(|()| process.launch())
                        .map(Some)
                } else {
                    Ok(None)
                }
            };

            match restart_result {
                Ok(restarted) => {
                    update_snapshot(&app, |snapshot| {
                        snapshot.tosu.memory_access = TosuMemoryAccess::Available;
                        snapshot.tosu.process_owned = restarted.is_some();
                        snapshot.tosu.connection = if snapshot.tracking_enabled {
                            TosuConnection::Searching
                        } else {
                            TosuConnection::Disabled
                        };
                        snapshot.tosu.message = Some(if restarted.is_some() {
                            "Memory access granted; Companion restarted tosu".into()
                        } else if externally_running {
                            "Memory access granted; restart the externally managed tosu process"
                                .into()
                        } else {
                            "Memory access granted; tosu can now read osu!lazer state".into()
                        });
                    })
                    .await;
                    Ok(())
                }
                Err(error) => {
                    let message =
                        format!("Memory access was granted, but tosu could not restart: {error}");
                    update_snapshot(&app, |snapshot| {
                        snapshot.tosu.memory_access = TosuMemoryAccess::Available;
                        snapshot.tosu.process_owned = false;
                        snapshot.tosu.message = Some(message.clone());
                    })
                    .await;
                    Err(message)
                }
            }
        }
        Err(error) => {
            let message = memory_access_message(&error);
            let memory_access = {
                let state = app.state::<AppState>();
                state
                    .process
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .memory_access_status()
            };
            update_snapshot(&app, |snapshot| {
                snapshot.tosu.memory_access = memory_access;
                snapshot.tosu.message = Some(message.clone());
            })
            .await;
            Err(message)
        }
    }
}

fn memory_access_message(error: &MemoryAccessError) -> String {
    match error {
        #[cfg(target_os = "linux")]
        MemoryAccessError::NotInstalled => "Install tosu before granting osu! memory access.".into(),
        #[cfg(target_os = "linux")]
        MemoryAccessError::UnsupportedLauncher => {
            "Companion could not locate the native binary behind the installed tosu launcher.".into()
        }
        #[cfg(target_os = "linux")]
        MemoryAccessError::ToolsUnavailable => {
            "Install pkexec and libcap to grant tosu memory access securely.".into()
        }
        #[cfg(target_os = "linux")]
        MemoryAccessError::AuthorizationDenied => {
            "Memory access was not granted. The system authorization prompt was cancelled or denied.".into()
        }
        #[cfg(target_os = "linux")]
        MemoryAccessError::Prompt(_) => "The system authorization prompt could not be opened.".into(),
        #[cfg(target_os = "linux")]
        MemoryAccessError::VerificationFailed => {
            "The system reported success, but tosu still lacks memory access.".into()
        }
        #[cfg(not(target_os = "linux"))]
        MemoryAccessError::UnsupportedPlatform => {
            "This platform does not require Linux ptrace access.".into()
        }
    }
}

#[tauri::command]
pub async fn connect_hanami(app: AppHandle) -> Result<(), String> {
    auth::connect(app)
        .await
        .map_err(|error| error.user_message())
}

#[tauri::command]
pub async fn disconnect_hanami(app: AppHandle) -> Result<(), String> {
    auth::logout(app)
        .await
        .map_err(|error| error.user_message())
}

#[tauri::command]
pub fn open_tosu_dashboard(app: AppHandle) -> Result<(), String> {
    let url = app
        .state::<AppState>()
        .settings
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .endpoint()
        .dashboard_url()
        .to_string();
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|_| "the tosu dashboard could not be opened".into())
}

#[tauri::command]
pub async fn select_tosu_executable(app: AppHandle) -> Result<(), String> {
    let selected = app
        .dialog()
        .file()
        .set_title("Select the tosu executable")
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(());
    };
    let path = selected
        .into_path()
        .map_err(|_| "the selected executable is not a local file".to_owned())?;
    if !TosuProcess::validate_executable(&path) {
        return Err("the selected file is not an executable tosu application".into());
    }

    {
        let state = app.state::<AppState>();
        state
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .set_executable(Some(path.clone()))?;
        state
            .process
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .set_configured_path(Some(path.clone()));
    }
    update_snapshot(&app, |snapshot| {
        snapshot.tosu.executable_available = true;
        snapshot.tosu.executable_path = Some(path.to_string_lossy().into_owned());
        snapshot.tosu.executable_configured = true;
        snapshot.tosu.message =
            Some("The selected tosu executable will be used when launching".into());
    })
    .await;
    Ok(())
}

#[tauri::command]
pub async fn reset_tosu_executable(app: AppHandle) -> Result<(), String> {
    let resolved = {
        let state = app.state::<AppState>();
        state
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .set_executable(None)?;
        let mut process = state
            .process
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        process.set_configured_path(None);
        process.resolved_executable()
    };
    update_snapshot(&app, |snapshot| {
        snapshot.tosu.executable_available = resolved.is_some();
        snapshot.tosu.executable_path = resolved
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned());
        snapshot.tosu.executable_configured = false;
        snapshot.tosu.message = Some("tosu executable discovery was reset to PATH".into());
    })
    .await;
    Ok(())
}

#[tauri::command]
pub fn open_repository() -> Result<(), String> {
    tauri_plugin_opener::open_url("https://github.com/hanami-osu/companion", None::<&str>)
        .map_err(|_| "the project repository could not be opened".into())
}

#[tauri::command]
pub async fn open_hanami_website(app: AppHandle) -> Result<(), String> {
    let url = current_snapshot(&app).await.auth.base_url;
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|_| "the Hanami website could not be opened".into())
}
