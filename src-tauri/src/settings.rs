use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::tosu::config::{DEFAULT_TOSU_PORT, TosuEndpoint};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct StoredSettings {
    tosu_port: u16,
    tosu_executable: Option<PathBuf>,
    tosu_auto_start: bool,
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            tosu_port: DEFAULT_TOSU_PORT,
            tosu_executable: None,
            tosu_auto_start: true,
        }
    }
}

pub struct AppSettings {
    path: PathBuf,
    values: StoredSettings,
}

impl AppSettings {
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let directory = app
            .path()
            .app_config_dir()
            .map_err(|error| format!("application settings directory is unavailable: {error}"))?;
        let path = directory.join("settings.json");
        let mut values = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("application settings are invalid: {error}"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => StoredSettings::default(),
            Err(error) => return Err(format!("application settings could not be read: {error}")),
        };
        if values.tosu_port == 0 {
            values.tosu_port = DEFAULT_TOSU_PORT;
        }
        Ok(Self { path, values })
    }

    pub fn endpoint(&self) -> TosuEndpoint {
        TosuEndpoint::new(self.values.tosu_port)
    }

    pub fn executable(&self) -> Option<PathBuf> {
        self.values.tosu_executable.clone()
    }

    pub fn tosu_auto_start(&self) -> bool {
        self.values.tosu_auto_start
    }

    pub fn set_executable(&mut self, executable: Option<PathBuf>) -> Result<(), String> {
        self.values.tosu_executable = executable;
        self.save()
    }

    pub fn set_tosu_auto_start(&mut self, enabled: bool) -> Result<(), String> {
        self.values.tosu_auto_start = enabled;
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(directory) = self.path.parent() else {
            return Err("application settings path is invalid".into());
        };
        fs::create_dir_all(directory).map_err(|error| {
            format!("application settings directory could not be created: {error}")
        })?;
        let bytes = serde_json::to_vec_pretty(&self.values)
            .map_err(|error| format!("application settings could not be encoded: {error}"))?;
        fs::write(&self.path, bytes)
            .map_err(|error| format!("application settings could not be saved: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_enable_tosu_auto_start_by_default() {
        let settings: StoredSettings =
            serde_json::from_str(r#"{"tosuPort":24050}"#).expect("legacy settings");
        assert!(settings.tosu_auto_start);
    }
}
