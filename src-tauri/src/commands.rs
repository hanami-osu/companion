use tauri::{AppHandle, Manager};

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
    })
    .await;
    Ok(())
}

#[tauri::command]
pub async fn launch_tosu(app: AppHandle) -> Result<(), String> {
    launch_tosu_impl(&app).await
}

pub async fn launch_tosu_impl(app: &AppHandle) -> Result<(), String> {
    let memory_access = TosuProcess::memory_access_status();
    if memory_access == TosuMemoryAccess::Required {
        let message =
            "Grant tosu Linux memory access before launching it from Companion".to_owned();
        update_snapshot(app, |snapshot| {
            snapshot.tosu.memory_access = memory_access;
            snapshot.tosu.message = Some(message.clone());
        })
        .await;
        return Err(message);
    }

    let snapshot = current_snapshot(app).await;
    if snapshot.tosu.connection == TosuConnection::Connected && !snapshot.tosu.process_owned {
        return Err("tosu is already running outside Companion".into());
    }

    let result = {
        let state = app.state::<AppState>();
        let mut process = state
            .process
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        process.launch()
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
                    "tosu is not installed or could not be found on PATH".to_owned()
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
            })
            .await;
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub async fn grant_tosu_memory_access(app: AppHandle) -> Result<(), String> {
    let result = tokio::task::spawn_blocking(TosuProcess::grant_memory_access)
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
                        snapshot.tosu.memory_access = TosuMemoryAccess::Granted;
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
                        snapshot.tosu.memory_access = TosuMemoryAccess::Granted;
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
            update_snapshot(&app, |snapshot| {
                snapshot.tosu.memory_access = TosuProcess::memory_access_status();
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
pub fn open_tosu_dashboard() -> Result<(), String> {
    tauri_plugin_opener::open_url("http://127.0.0.1:24050", None::<&str>)
        .map_err(|_| "the tosu dashboard could not be opened".into())
}

#[tauri::command]
pub async fn open_hanami_website(app: AppHandle) -> Result<(), String> {
    let url = current_snapshot(&app).await.auth.base_url;
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|_| "the Hanami website could not be opened".into())
}
