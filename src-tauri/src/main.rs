// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
fn configure_linux_renderer() {
    use std::env;

    if env::var_os("HANAMI_FORCE_WAYLAND").is_some() {
        unsafe {
            env::set_var("GDK_BACKEND", "wayland");
        }
        return;
    }

    if env::var_os("HANAMI_FORCE_X11").is_some() {
        unsafe {
            env::set_var("GDK_BACKEND", "x11");
        }
        return;
    }

    let session_type = env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_ascii_lowercase();

    if session_type != "wayland" {
        return;
    }

    let desktop = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();

    let is_gnome = desktop.contains("gnome");
    let xwayland_available = env::var_os("DISPLAY").is_some();

    if !is_gnome && xwayland_available {
        // WebKitGTK native decorations are unreliable on some non-GNOME Wayland
        // compositors. Prefer XWayland there while preserving user overrides.
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
