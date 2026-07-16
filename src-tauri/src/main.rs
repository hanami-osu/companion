// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
fn configure_linux_renderer() {
    use std::env;

    // manual override for people who like wayland for some reason.
    if env::var_os("HANAMI_FORCE_WAYLAND").is_some() {
        return;
    }
    // we don't care about errors here.
    let session_type = env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_ascii_lowercase();

    // return early since it's already x11.
    if session_type != "wayland" {
        return;
    }

    let desktop = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();

    let is_gnome = desktop.contains("gnome");
    let xwayland_available = env::var_os("DISPLAY").is_some();

    if !is_gnome && xwayland_available {
        println!("working in x11.");

        unsafe {
            env::set_var("GDK_BACKEND", "x11");
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    configure_linux_renderer();

    hanami_companion_lib::run()
}
