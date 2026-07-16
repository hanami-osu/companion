use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::tosu::config::{DEFAULT_TOSU_PORT, TosuEndpoint};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
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
            tosu_auto_start: false,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawStoredSettings {
    tosu_port: Option<u64>,
    tosu_executable: Option<PathBuf>,
    tosu_auto_start: Option<bool>,
}

pub struct AppSettings {
    path: PathBuf,
    values: StoredSettings,
    warning: Option<String>,
}

impl AppSettings {
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let directory = app
            .path()
            .app_config_dir()
            .map_err(|error| format!("application settings directory is unavailable: {error}"))?;
        Ok(Self::load_from_path(directory.join("settings.json")))
    }

    fn load_from_path(path: PathBuf) -> Self {
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Self {
                    path,
                    values: StoredSettings::default(),
                    warning: None,
                };
            }
            Err(error) => {
                return Self {
                    path,
                    values: StoredSettings::default(),
                    warning: Some(format!(
                        "Settings could not be read; safe defaults are active: {error}"
                    )),
                };
            }
        };

        let raw = match serde_json::from_slice::<RawStoredSettings>(&bytes) {
            Ok(raw) => raw,
            Err(_) => {
                let preserved = preserve_invalid_file(&path);
                let warning = match preserved {
                    Ok(preserved) => format!(
                        "Malformed settings were preserved as {}; safe defaults are active.",
                        preserved.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    Err(error) => format!(
                        "Settings are malformed and could not be preserved ({error}); safe defaults are active."
                    ),
                };
                return Self {
                    path,
                    values: StoredSettings::default(),
                    warning: Some(warning),
                };
            }
        };

        let mut warnings = Vec::new();
        let tosu_port = match raw.tosu_port {
            None => DEFAULT_TOSU_PORT,
            Some(0) => {
                warnings.push("A legacy zero tosu port was reset to 24050.");
                DEFAULT_TOSU_PORT
            }
            Some(port @ 1..=65_535) => port as u16,
            Some(_) => {
                warnings.push("An out-of-range tosu port was reset to 24050.");
                DEFAULT_TOSU_PORT
            }
        };
        let tosu_executable = raw.tosu_executable.and_then(|path| {
            if valid_executable_path_shape(&path) {
                Some(path)
            } else {
                warnings.push("An invalid configured tosu executable path was ignored.");
                None
            }
        });

        Self {
            path,
            values: StoredSettings {
                tosu_port,
                tosu_executable,
                // Legacy files without this field retain the previous behavior. New files default off.
                tosu_auto_start: raw.tosu_auto_start.unwrap_or(true),
            },
            warning: (!warnings.is_empty()).then(|| warnings.join(" ")),
        }
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

    pub fn warning(&self) -> Option<String> {
        self.warning.clone()
    }

    pub fn set_executable(&mut self, executable: Option<PathBuf>) -> Result<(), String> {
        if executable
            .as_deref()
            .is_some_and(|path| !valid_executable_path_shape(path))
        {
            return Err("the configured tosu executable path is invalid".into());
        }
        let mut next = self.values.clone();
        next.tosu_executable = executable;
        self.save_values(&next)?;
        self.values = next;
        Ok(())
    }

    pub fn set_tosu_auto_start(&mut self, enabled: bool) -> Result<(), String> {
        let mut next = self.values.clone();
        next.tosu_auto_start = enabled;
        self.save_values(&next)?;
        self.values = next;
        Ok(())
    }

    fn save_values(&self, values: &StoredSettings) -> Result<(), String> {
        let Some(directory) = self.path.parent() else {
            return Err("application settings path is invalid".into());
        };
        fs::create_dir_all(directory).map_err(|error| {
            format!("application settings directory could not be created: {error}")
        })?;
        let bytes = serde_json::to_vec_pretty(values)
            .map_err(|error| format!("application settings could not be encoded: {error}"))?;
        let temporary = directory.join(format!(".settings.json.tmp-{}", Uuid::new_v4()));
        let result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| format!("temporary settings could not be created: {error}"))?;
            file.write_all(&bytes)
                .map_err(|error| format!("temporary settings could not be written: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("temporary settings could not be flushed: {error}"))?;
            drop(file);
            fs::rename(&temporary, &self.path)
                .map_err(|error| format!("application settings could not be replaced: {error}"))?;
            sync_directory(directory);
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn valid_executable_path_shape(path: &Path) -> bool {
    path.is_absolute() && path.file_name().is_some_and(|name| !name.is_empty())
}

fn preserve_invalid_file(path: &Path) -> Result<PathBuf, std::io::Error> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let preserved = path.with_file_name(format!(
        "{file_name}.invalid-{timestamp}-{}",
        Uuid::new_v4()
    ));
    fs::rename(path, &preserved)?;
    Ok(preserved)
}

#[cfg(unix)]
fn sync_directory(directory: &Path) {
    let _ = File::open(directory).and_then(|file| file.sync_all());
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("hanami-settings-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("test directory");
            Self(path)
        }

        fn settings_path(&self) -> PathBuf {
            self.0.join("settings.json")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_file_uses_safe_new_install_defaults() {
        let directory = TestDirectory::new();
        let settings = AppSettings::load_from_path(directory.settings_path());
        assert_eq!(settings.endpoint().port(), DEFAULT_TOSU_PORT);
        assert!(!settings.tosu_auto_start());
        assert!(settings.warning().is_none());
    }

    #[test]
    fn valid_file_loads_and_legacy_missing_auto_start_stays_enabled() {
        let directory = TestDirectory::new();
        fs::write(
            directory.settings_path(),
            r#"{"tosuPort":24051,"tosuExecutable":"/opt/tosu/tosu","tosuAutoStart":false}"#,
        )
        .expect("settings");
        let settings = AppSettings::load_from_path(directory.settings_path());
        assert_eq!(settings.endpoint().port(), 24_051);
        assert!(!settings.tosu_auto_start());

        fs::write(directory.settings_path(), r#"{"tosuPort":24050}"#).expect("legacy settings");
        let legacy = AppSettings::load_from_path(directory.settings_path());
        assert!(legacy.tosu_auto_start());
    }

    #[test]
    fn malformed_file_is_preserved_and_does_not_block_startup() {
        let directory = TestDirectory::new();
        let path = directory.settings_path();
        let malformed = b"{not valid json";
        fs::write(&path, malformed).expect("malformed settings");
        let settings = AppSettings::load_from_path(path.clone());
        assert!(!settings.tosu_auto_start());
        assert!(settings.warning().is_some());
        assert!(!path.exists());
        let preserved = fs::read_dir(&directory.0)
            .expect("directory")
            .find_map(Result::ok)
            .expect("preserved settings")
            .path();
        assert_eq!(fs::read(preserved).expect("preserved bytes"), malformed);
    }

    #[test]
    fn invalid_ports_migrate_to_the_default() {
        for value in [0_u64, 65_536, u64::MAX] {
            let directory = TestDirectory::new();
            fs::write(
                directory.settings_path(),
                format!(r#"{{"tosuPort":{value},"tosuAutoStart":false}}"#),
            )
            .expect("settings");
            let settings = AppSettings::load_from_path(directory.settings_path());
            assert_eq!(settings.endpoint().port(), DEFAULT_TOSU_PORT);
            assert!(settings.warning().is_some());
        }
    }

    #[test]
    fn atomic_save_replaces_a_complete_existing_file() {
        let directory = TestDirectory::new();
        let path = directory.settings_path();
        fs::write(&path, r#"{"tosuAutoStart":true}"#).expect("initial settings");
        let mut settings = AppSettings::load_from_path(path.clone());
        settings.set_tosu_auto_start(false).expect("atomic save");
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(path).expect("saved settings")).expect("valid JSON");
        assert_eq!(saved["tosuAutoStart"], false);
        assert!(fs::read_dir(&directory.0).expect("directory").all(|entry| {
            !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp-")
        }));
    }
}
