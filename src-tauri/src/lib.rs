mod activity;
mod app_state;
mod auth;
mod commands;
pub mod hanami;
mod tosu;
mod tray;

use app_state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::set_tracking_enabled,
            commands::launch_tosu,
            commands::stop_owned_tosu,
            commands::grant_tosu_memory_access,
            commands::connect_hanami,
            commands::disconnect_hanami,
            commands::open_tosu_dashboard,
            commands::open_hanami_website,
        ])
        .setup(|app| {
            tray::setup(app)?;
            tosu::start_listener(app.handle().clone());
            auth::start_supervisor(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build Hanami Companion");

    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            shutdown(app);
        }
    });
}

pub(crate) fn shutdown(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    if !state.begin_shutdown() {
        return;
    }
    let mut process = state
        .process
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    process.stop_owned_on_shutdown();
}
