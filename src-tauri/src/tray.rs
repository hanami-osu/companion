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
    let label = match snapshot.tosu.connection {
        TosuConnection::Disabled => "tosu: tracking disabled",
        TosuConnection::Searching => "tosu: searching",
        TosuConnection::Connecting => "tosu: connecting",
        TosuConnection::Connected => "tosu: connected",
        TosuConnection::Unavailable => "tosu: unavailable",
        TosuConnection::Error => "tosu: connection error",
    };
    let _ = menu.status.set_text(label);
    let _ = menu.tracking.set_checked(snapshot.tracking_enabled);
    let _ = menu.launch_tosu.set_enabled(
        snapshot.tosu.connection != TosuConnection::Connected
            && !snapshot.tosu.process_owned
            && snapshot.tosu.memory_access != TosuMemoryAccess::Required,
    );
    let _ = menu.stop_tosu.set_enabled(snapshot.tosu.process_owned);
    let _ = menu.sign_out.set_enabled(matches!(
        snapshot.auth.state,
        AuthState::SignedIn | AuthState::Refreshing
    ));
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
