use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex as AsyncMutex, RwLock, watch};

use crate::{
    auth::{AuthConfig, AuthRuntime},
    settings::AppSettings,
    tosu::process::{TosuMemoryAccess, TosuProcess},
};

pub const SNAPSHOT_EVENT: &str = "companion://snapshot";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TosuConnection {
    Disabled,
    #[default]
    Searching,
    Connecting,
    Connected,
    Stale,
    Unavailable,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TosuStatus {
    pub connection: TosuConnection,
    pub process_owned: bool,
    pub executable_available: bool,
    pub executable_path: Option<String>,
    pub executable_configured: bool,
    pub auto_start: bool,
    pub port: u16,
    pub memory_access: TosuMemoryAccess,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OsuState {
    Menu,
    SongSelect,
    Gameplay,
    Results,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsuStatus {
    pub running: bool,
    pub state: OsuState,
    pub client: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ruleset {
    Osu,
    Taiko,
    Catch,
    Mania,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatmapSummary {
    pub beatmap_id: Option<u64>,
    pub beatmap_set_id: Option<u64>,
    pub checksum: Option<String>,
    pub artist: String,
    pub title: String,
    pub difficulty: String,
    pub mapper: String,
    pub ruleset: Ruleset,
    pub background_url: Option<String>,
    pub max_combo: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionMod {
    pub acronym: String,
    pub settings: BTreeMap<String, Value>,
}

impl From<&str> for CompanionMod {
    fn from(acronym: &str) -> Self {
        Self {
            acronym: acronym.to_owned(),
            settings: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    #[serde(flatten)]
    pub beatmap: BeatmapSummary,
    pub mods: Vec<CompanionMod>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HitCounts {
    pub great: u32,
    pub ok: u32,
    pub meh: u32,
    pub katu: u32,
    pub geki: u32,
    pub misses: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePlay {
    pub player_name: Option<String>,
    pub score: u64,
    pub accuracy: f64,
    pub combo: u32,
    pub maximum_combo: Option<u32>,
    pub hits: HitCounts,
    pub current_pp: Option<f64>,
    pub maximum_pp: Option<f64>,
    pub progress: f64,
    pub elapsed_seconds: f64,
    pub failed: bool,
    pub rank: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    #[default]
    SignedOut,
    OpeningBrowser,
    WaitingForApproval,
    ExchangingCode,
    SignedIn,
    Refreshing,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub state: AuthState,
    pub message: Option<String>,
    pub base_url: String,
    pub is_production: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionSnapshot {
    pub tracking_enabled: bool,
    pub tosu: TosuStatus,
    pub osu: OsuStatus,
    pub now_playing: Option<NowPlaying>,
    pub live_play: Option<LivePlay>,
    pub recent_activity: Vec<crate::activity::model::RecentPlay>,
    pub auth: AuthStatus,
    pub app_version: String,
    pub upload_available: bool,
}

pub struct AppState {
    pub snapshot: RwLock<CompanionSnapshot>,
    pub tracking: watch::Sender<bool>,
    pub process: Mutex<TosuProcess>,
    pub settings: Mutex<AppSettings>,
    pub auth: AsyncMutex<AuthRuntime>,
    pub tosu_listener_started: AtomicBool,
    pub auth_supervisor_started: AtomicBool,
    pub auth_flow_active: AtomicBool,
    pub shutting_down: AtomicBool,
}

impl AppState {
    pub fn new(settings: AppSettings) -> Self {
        let (tracking, _) = watch::channel(true);
        let auth_config = AuthConfig::from_environment();
        let endpoint = settings.endpoint();
        let process = TosuProcess::new(settings.executable());
        let resolved_executable = process.resolved_executable();
        let executable_available = resolved_executable.is_some();
        let memory_access = process.memory_access_status();

        Self {
            snapshot: RwLock::new(CompanionSnapshot {
                tracking_enabled: true,
                tosu: TosuStatus {
                    connection: TosuConnection::Searching,
                    process_owned: false,
                    executable_available,
                    executable_path: resolved_executable
                        .as_deref()
                        .map(|path| path.to_string_lossy().into_owned()),
                    executable_configured: settings.executable().is_some(),
                    auto_start: settings.tosu_auto_start(),
                    port: endpoint.port(),
                    memory_access,
                    message: None,
                },
                osu: OsuStatus::default(),
                now_playing: None,
                live_play: None,
                recent_activity: Vec::new(),
                auth: AuthStatus {
                    state: AuthState::SignedOut,
                    message: None,
                    base_url: auth_config.base_url().to_owned(),
                    is_production: auth_config.is_production(),
                },
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
                upload_available: false,
            }),
            tracking,
            process: Mutex::new(process),
            settings: Mutex::new(settings),
            auth: AsyncMutex::new(AuthRuntime::new(auth_config)),
            tosu_listener_started: AtomicBool::new(false),
            auth_supervisor_started: AtomicBool::new(false),
            auth_flow_active: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
        }
    }

    pub fn begin_shutdown(&self) -> bool {
        !self.shutting_down.swap(true, Ordering::SeqCst)
    }
}

pub async fn current_snapshot(app: &AppHandle) -> CompanionSnapshot {
    app.state::<AppState>().snapshot.read().await.clone()
}

pub async fn update_snapshot<F>(app: &AppHandle, update: F) -> CompanionSnapshot
where
    F: FnOnce(&mut CompanionSnapshot),
{
    let snapshot = {
        let state = app.state::<AppState>();
        let mut snapshot = state.snapshot.write().await;
        update(&mut snapshot);
        snapshot.clone()
    };

    let _ = app.emit(SNAPSHOT_EVENT, &snapshot);
    crate::tray::sync_menu(app, &snapshot);
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_snapshot_contract_uses_the_frontend_field_names() {
        let snapshot = CompanionSnapshot {
            tracking_enabled: true,
            tosu: TosuStatus {
                connection: TosuConnection::Stale,
                process_owned: false,
                executable_available: true,
                executable_path: Some("/usr/bin/tosu".into()),
                executable_configured: false,
                auto_start: true,
                port: 24_050,
                memory_access: TosuMemoryAccess::PossiblyRequired,
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
        };

        let value = serde_json::to_value(snapshot).expect("snapshot serialization");
        let object = value.as_object().expect("snapshot object");
        for key in [
            "trackingEnabled",
            "tosu",
            "osu",
            "nowPlaying",
            "livePlay",
            "recentActivity",
            "auth",
            "appVersion",
            "uploadAvailable",
        ] {
            assert!(object.contains_key(key), "missing contract key {key}");
        }
        assert_eq!(value["tosu"]["connection"], "stale");
        assert_eq!(value["tosu"]["memoryAccess"], "possibly_required");
        assert_eq!(value["tosu"]["port"], 24_050);
        assert_eq!(value["tosu"]["executableConfigured"], false);
        assert_eq!(value["tosu"]["autoStart"], true);
        assert_eq!(value["auth"]["state"], "signed_out");
    }
}
