// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

mod hanami_api;
mod tosu;
mod ws_client;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(tosu::TosuState {
            process: std::sync::Mutex::new(None),
        })
        .manage(tokio::sync::Mutex::new(hanami_api::HanamiClient::new()))
        .invoke_handler(tauri::generate_handler![
            greet,
            tosu::toggle_tosu,
            tosu::is_tosu_running
        ])
        .setup(|app| {
            ws_client::start_tosu_listener(app.handle().clone());
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let toggle_i = MenuItem::with_id(app, "toggle", "Show/Hide", true, None::<&str>)?;
            let toggle_t = MenuItem::with_id(
                app,
                "toggle-tosu",
                "Enable/Disable tosu",
                true,
                None::<&str>,
            )?;
            let menu = Menu::with_items(app, &[&toggle_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        // kill tosu first, then exit the app
                        let state = app.state::<tosu::TosuState>();
                        if let Some(p) = state.process.lock().unwrap().take() {
                            let _ = p.kill();
                        }

                        app.exit(0);
                    }
                    "toggle" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
