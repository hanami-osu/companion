use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_shell::{process::CommandChild, ShellExt};

pub struct TosuState {
    pub process: Mutex<Option<CommandChild>>,
}

#[tauri::command]
pub fn toggle_tosu(
    app: AppHandle,
    state: State<'_, TosuState>,
    enable: bool,
) -> Result<bool, String> {
    let mut process = state.process.lock().unwrap();

    if enable {
        if process.is_none() {
            // TODO: automatically install and set up tosu if not found in the system.
            let (_, child) = app
                .shell()
                .command("tosu")
                .spawn()
                .map_err(|e| format!("Failed to spawn tosu: {}", e))?;

            *process = Some(child);
            return Ok(true);
        }
        Ok(true)
    } else {
        if let Some(p) = process.take() {
            let _ = p.kill();
        }
        Ok(false)
    }
}

#[tauri::command]
pub fn is_tosu_running(state: State<'_, TosuState>) -> Result<bool, String> {
    let process = state.process.lock().unwrap();
    Ok(process.is_some())
}
