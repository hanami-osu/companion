use std::sync::Mutex;
use tauri::{
    App, AppHandle, Manager,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{
    app_state::{AuthState, CompanionSnapshot, TosuConnection},
    commands,
    tosu::process::TosuMemoryAccess,
};

pub struct TrayMenuState {
    status: MenuItem<tauri::Wry>,
    tracking: CheckMenuItem<tauri::Wry>,
    launch_tosu: MenuItem<tauri::Wry>,
    stop_tosu: MenuItem<tauri::Wry>,
    sign_out: MenuItem<tauri::Wry>,
    last_state: Mutex<Option<TrayState>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrayState {
    tracking_enabled: bool,
    connection: TosuConnection,
    process_owned: bool,
    signed_in: bool,
    memory_access: TosuMemoryAccess,
}

impl TrayState {
    fn from_snapshot(snapshot: &CompanionSnapshot) -> Self {
        Self {
            tracking_enabled: snapshot.tracking_enabled,
            connection: snapshot.tosu.connection.clone(),
            process_owned: snapshot.tosu.process_owned,
            signed_in: matches!(
                snapshot.auth.state,
                AuthState::SignedIn | AuthState::Refreshing
            ),
            memory_access: snapshot.tosu.memory_access,
        }
    }
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Hanami Companion", true, None::<&str>)?;
    let status = MenuItem::with_id(app, "status", "tosu: searching", false, None::<&str>)?;
    let tracking = CheckMenuItem::with_id(
        app,
        "tracking",
        "Tracking enabled",
        true,
        true,
        None::<&str>,
    )?;
    let launch_tosu = MenuItem::with_id(app, "launch-tosu", "Launch tosu", true, None::<&str>)?;
    let stop_tosu = MenuItem::with_id(
        app,
        "stop-tosu",
        "Stop Companion-owned tosu",
        false,
        None::<&str>,
    )?;
    let sign_out = MenuItem::with_id(app, "sign-out", "Sign out of Hanami", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Hanami Companion", true, None::<&str>)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &status,
            &separator_one,
            &tracking,
            &launch_tosu,
            &stop_tosu,
            &sign_out,
            &separator_two,
            &quit,
        ],
    )?;

    app.manage(TrayMenuState {
        status,
        tracking,
        launch_tosu,
        stop_tosu,
        sign_out,
        last_state: Mutex::new(None),
    });

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().expect("application icon").clone())
        .tooltip("Hanami Companion")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "open" => show_main_window(app),
        "tracking" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let enabled = !crate::app_state::current_snapshot(&app)
                    .await
                    .tracking_enabled;
                let _ = commands::set_tracking_enabled_impl(&app, enabled).await;
            });
        }
        "launch-tosu" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = commands::launch_tosu_impl(&app).await;
            });
        }
        "stop-tosu" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = commands::stop_owned_tosu_impl(&app).await;
            });
        }
        "sign-out" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::auth::logout(app).await;
            });
        }
        "quit" => {
            crate::shutdown(app);
            app.exit(0);
        }
        _ => {}
    }
}

pub fn sync_menu(app: &AppHandle, snapshot: &CompanionSnapshot) {
    let Some(menu) = app.try_state::<TrayMenuState>() else {
        return;
    };
    let next = TrayState::from_snapshot(snapshot);
    {
        let mut last = menu
            .last_state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if last.as_ref() == Some(&next) {
            return;
        }
        *last = Some(next.clone());
    }
    let label = match next.connection {
        TosuConnection::Disabled => "tosu: tracking disabled",
        TosuConnection::Searching => "tosu: searching",
        TosuConnection::Connecting => "tosu: connecting",
        TosuConnection::Connected => "tosu: connected",
        TosuConnection::Stale => "tosu: stale; reconnecting",
        TosuConnection::Unavailable => "tosu: unavailable",
        TosuConnection::Error => "tosu: connection error",
    };
    let _ = menu.status.set_text(label);
    let _ = menu.tracking.set_checked(next.tracking_enabled);
    let _ = menu.launch_tosu.set_enabled(
        next.tracking_enabled
            && !matches!(
                next.connection,
                TosuConnection::Connected | TosuConnection::Connecting | TosuConnection::Stale
            )
            && !next.process_owned,
    );
    let _ = menu.stop_tosu.set_enabled(next.process_owned);
    let _ = menu.sign_out.set_enabled(next.signed_in);
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::{
        AuthStatus, CompanionSnapshot, HitCounts, LivePlay, OsuStatus, TosuStatus,
    };

    fn snapshot() -> CompanionSnapshot {
        CompanionSnapshot {
            tracking_enabled: true,
            tosu: TosuStatus {
                connection: TosuConnection::Connected,
                process_owned: false,
                executable_available: true,
                executable_path: Some("/usr/bin/tosu".into()),
                executable_configured: false,
                auto_start: true,
                port: 24_050,
                memory_access: TosuMemoryAccess::Unknown,
                message: None,
            },
            osu: OsuStatus::default(),
            now_playing: None,
            live_play: None,
            recent_activity: Vec::new(),
            auth: AuthStatus {
                state: AuthState::SignedOut,
                message: None,
                base_url: "https://hanami.yorunoken.com".into(),
                is_production: true,
            },
            app_version: "0.1.0".into(),
            upload_available: false,
        }
    }

    #[test]
    fn gameplay_only_changes_do_not_change_tray_state() {
        let mut changed = snapshot();
        let original = TrayState::from_snapshot(&changed);
        changed.live_play = Some(LivePlay {
            score: 999,
            combo: 12,
            hits: HitCounts::default(),
            ..LivePlay::default()
        });
        assert_eq!(original, TrayState::from_snapshot(&changed));
    }

    #[test]
    fn relevant_connection_changes_change_tray_state() {
        let mut changed = snapshot();
        let original = TrayState::from_snapshot(&changed);
        changed.tosu.connection = TosuConnection::Stale;
        assert_ne!(original, TrayState::from_snapshot(&changed));
    }
}
