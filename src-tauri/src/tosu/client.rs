use std::{sync::atomic::Ordering, time::Duration};

use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Manager};
use tokio::time::{Instant, MissedTickBehavior};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    activity::ActivityTracker,
    app_state::{
        AppState, OsuState, SNAPSHOT_EVENT, TosuConnection, current_snapshot, update_snapshot,
    },
    tosu::{config::TosuEndpoint, process::TosuMemoryAccess},
};

use super::model::{PayloadError, RawTosuPayload};

const UI_INTERVAL: Duration = Duration::from_millis(125);
const MAX_BACKOFF: Duration = Duration::from_secs(8);
const MAX_ACTIVITY_SILENCE: Duration = Duration::from_secs(15);
const MAX_VALID_STATE_SILENCE: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MonitorEnd {
    TrackingDisabled,
    Disconnected,
    Stale,
    Incompatible,
}

struct ConnectionHealth {
    opened_at: Instant,
    last_activity: Option<Instant>,
    last_valid_state: Option<Instant>,
}

impl ConnectionHealth {
    fn new(now: Instant) -> Self {
        Self {
            opened_at: now,
            last_activity: None,
            last_valid_state: None,
        }
    }

    fn record_activity(&mut self, now: Instant) {
        self.last_activity = Some(now);
    }

    fn record_valid_state(&mut self, now: Instant) {
        self.last_activity = Some(now);
        self.last_valid_state = Some(now);
    }

    fn transport_is_stale(&self, now: Instant) -> bool {
        let activity_anchor = self.last_activity.unwrap_or(self.opened_at);
        now.duration_since(activity_anchor) >= MAX_ACTIVITY_SILENCE
    }

    fn valid_state_timed_out(&self, now: Instant) -> bool {
        let valid_anchor = self.last_valid_state.unwrap_or(self.opened_at);
        now.duration_since(valid_anchor) >= MAX_VALID_STATE_SILENCE
    }
}

pub fn start_listener(app: AppHandle) {
    let state = app.state::<AppState>();
    if state.tosu_listener_started.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(run_listener(app));
}

async fn run_listener(app: AppHandle) {
    let mut tracking = app.state::<AppState>().tracking.subscribe();
    let mut backoff = Duration::from_millis(500);
    let mut activity = ActivityTracker::default();

    loop {
        if app.state::<AppState>().shutting_down.load(Ordering::SeqCst) {
            break;
        }

        if !*tracking.borrow() {
            activity.reset();
            set_inactive(&app, TosuConnection::Disabled, None).await;
            tokio::select! {
                changed = tracking.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    refresh_process_state(&app).await;
                }
            }
            backoff = Duration::from_millis(500);
            continue;
        }

        refresh_process_state(&app).await;
        set_connection(&app, TosuConnection::Searching, None).await;
        let endpoint = current_endpoint(&app);
        let probe = tokio::time::timeout(
            Duration::from_millis(700),
            tokio::net::TcpStream::connect(endpoint.socket_addr()),
        )
        .await;

        if !matches!(probe, Ok(Ok(_))) {
            activity.reset();
            set_unavailable(&app).await;
            if wait_or_tracking_change(&mut tracking, backoff).await {
                continue;
            }
            backoff = (backoff * 2).min(MAX_BACKOFF);
            continue;
        }

        set_connection(&app, TosuConnection::Connecting, None).await;
        match tokio::time::timeout(
            Duration::from_secs(3),
            connect_async(endpoint.websocket_url().as_str()),
        )
        .await
        {
            Ok(Ok((socket, _))) => {
                let outcome =
                    monitor_socket(&app, socket, &mut tracking, &mut activity, &endpoint).await;
                activity.reset();
                match outcome {
                    MonitorEnd::TrackingDisabled => continue,
                    MonitorEnd::Disconnected => mark_disconnected(&app).await,
                    MonitorEnd::Stale | MonitorEnd::Incompatible => {}
                }
                backoff = if outcome == MonitorEnd::Disconnected {
                    Duration::from_millis(500)
                } else {
                    (backoff * 2).min(MAX_BACKOFF)
                };
            }
            Ok(Err(error)) => {
                activity.reset();
                eprintln!("tosu WebSocket connection failed: {error}");
                set_inactive(
                    &app,
                    TosuConnection::Error,
                    Some("tosu accepted a connection but its v2 WebSocket was unavailable".into()),
                )
                .await;
            }
            Err(_) => {
                activity.reset();
                set_inactive(
                    &app,
                    TosuConnection::Error,
                    Some("the tosu v2 WebSocket connection timed out".into()),
                )
                .await;
            }
        }

        if !*tracking.borrow() {
            continue;
        }
        if !wait_or_tracking_change(&mut tracking, backoff).await {
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }
}

async fn monitor_socket<S>(
    app: &AppHandle,
    socket: tokio_tungstenite::WebSocketStream<S>,
    tracking: &mut tokio::sync::watch::Receiver<bool>,
    activity: &mut ActivityTracker,
    endpoint: &TosuEndpoint,
) -> MonitorEnd
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut writer, mut reader) = socket.split();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut watchdog = tokio::time::interval(Duration::from_secs(1));
    watchdog.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut process_poll = tokio::time::interval(Duration::from_secs(1));
    process_poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_ui_emit = Instant::now() - UI_INTERVAL;
    let mut last_parse_log: Option<Instant> = None;
    let mut consecutive_unexpected = 0_u8;
    let mut health = ConnectionHealth::new(Instant::now());
    let mut idle_state_applied = false;

    loop {
        tokio::select! {
            changed = tracking.changed() => {
                if changed.is_err() || !*tracking.borrow() {
                    let _ = writer.send(Message::Close(None)).await;
                    return MonitorEnd::TrackingDisabled;
                }
            }
            _ = heartbeat.tick() => {
                if writer.send(Message::Ping(Vec::new())).await.is_err() {
                    return MonitorEnd::Disconnected;
                }
            }
            _ = watchdog.tick() => {
                let now = Instant::now();
                if health.transport_is_stale(now) {
                    let _ = writer.send(Message::Close(None)).await;
                    set_inactive(
                        app,
                        TosuConnection::Stale,
                        Some("the tosu connection stopped responding; reconnecting".into()),
                    ).await;
                    return MonitorEnd::Stale;
                }
                if !idle_state_applied && health.valid_state_timed_out(now) {
                    activity.reset();
                    set_connected_without_osu(app).await;
                    idle_state_applied = true;
                }
            }
            _ = process_poll.tick() => refresh_process_state(app).await,
            message = reader.next() => {
                let now = Instant::now();
                match message {
                    Some(Ok(Message::Text(payload))) => {
                        health.record_activity(now);
                        match RawTosuPayload::parse(&payload) {
                            Ok(raw) => {
                                health.record_valid_state(now);
                                idle_state_applied = false;
                                consecutive_unexpected = 0;
                                let normalized = raw.normalize(endpoint);
                                let recent = activity.observe(normalized.observation);
                                {
                                    let state = app.state::<AppState>();
                                    let mut snapshot = state.snapshot.write().await;
                                    snapshot.tosu.connection = TosuConnection::Connected;
                                    snapshot.tosu.message = None;
                                    snapshot.osu = normalized.osu;
                                    snapshot.now_playing = normalized.now_playing;
                                    snapshot.live_play = normalized.live_play;
                                    snapshot.recent_activity = recent;
                                }
                                if last_ui_emit.elapsed() >= UI_INTERVAL {
                                    emit_current(app).await;
                                    last_ui_emit = Instant::now();
                                }
                            }
                            Err(PayloadError::NotReady(error)) => {
                                health.record_valid_state(now);
                                idle_state_applied = true;
                                consecutive_unexpected = 0;
                                activity.reset();
                                set_connected_without_osu(app).await;
                                if indicates_memory_access_failure(&error) {
                                    mark_memory_access_possibly_required(app).await;
                                }
                                if last_ui_emit.elapsed() >= UI_INTERVAL {
                                    emit_current(app).await;
                                    last_ui_emit = Instant::now();
                                }
                            }
                            Err(error) => {
                                consecutive_unexpected = consecutive_unexpected.saturating_add(1);
                                if last_parse_log.is_none_or(|last| last.elapsed() >= Duration::from_secs(30)) {
                                    eprintln!("ignored incompatible tosu v2 payload: {error}");
                                    last_parse_log = Some(Instant::now());
                                }
                                if consecutive_unexpected >= 3 {
                                    let _ = writer.send(Message::Close(None)).await;
                                    set_inactive(
                                        app,
                                        TosuConnection::Error,
                                        Some("tosu repeatedly returned an unexpected v2 payload".into()),
                                    ).await;
                                    return MonitorEnd::Incompatible;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Pong(_))) | Some(Ok(Message::Binary(_))) => {
                        health.record_activity(now);
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                        return MonitorEnd::Disconnected;
                    }
                    Some(Ok(_)) => health.record_activity(now),
                }
            }
        }
    }
}

fn current_endpoint(app: &AppHandle) -> TosuEndpoint {
    app.state::<AppState>()
        .settings
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .endpoint()
}

async fn emit_current(app: &AppHandle) {
    use tauri::Emitter;
    let snapshot = current_snapshot(app).await;
    let _ = app.emit(SNAPSHOT_EVENT, &snapshot);
    crate::tray::sync_menu(app, &snapshot);
}

async fn refresh_process_state(app: &AppHandle) {
    let (owned, executable, executable_configured, memory_access) = {
        let state = app.state::<AppState>();
        let mut process = state
            .process
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let owned = process.is_owned();
        let executable = process.resolved_executable();
        let executable_configured = process.configured_path().is_some();
        let mut memory_access = process.memory_access_status();
        if process.memory_failure_observed() && memory_access != TosuMemoryAccess::Available {
            memory_access = TosuMemoryAccess::PossiblyRequired;
        }
        (owned, executable, executable_configured, memory_access)
    };
    let current = current_snapshot(app).await;
    let executable_path = executable
        .as_deref()
        .map(|path| path.to_string_lossy().into_owned());
    if current.tosu.process_owned != owned
        || current.tosu.executable_available != executable.is_some()
        || current.tosu.executable_path != executable_path
        || current.tosu.executable_configured != executable_configured
        || current.tosu.memory_access != memory_access
    {
        update_snapshot(app, |snapshot| {
            snapshot.tosu.process_owned = owned;
            snapshot.tosu.executable_available = executable.is_some();
            snapshot.tosu.executable_path = executable_path;
            snapshot.tosu.executable_configured = executable_configured;
            snapshot.tosu.memory_access = memory_access;
        })
        .await;
    }
}

async fn set_connection(app: &AppHandle, connection: TosuConnection, message: Option<String>) {
    let current = current_snapshot(app).await;
    if current.tosu.connection == connection && current.tosu.message == message {
        return;
    }
    update_snapshot(app, |snapshot| {
        snapshot.tracking_enabled = true;
        snapshot.tosu.connection = connection;
        snapshot.tosu.message = message;
    })
    .await;
}

async fn set_connected_without_osu(app: &AppHandle) {
    let current = current_snapshot(app).await;
    if current.tosu.connection == TosuConnection::Connected
        && !current.osu.running
        && current.now_playing.is_none()
        && current.live_play.is_none()
    {
        return;
    }
    update_snapshot(app, |snapshot| {
        snapshot.tosu.connection = TosuConnection::Connected;
        snapshot.tosu.message = None;
        clear_displayed_osu_state(snapshot);
    })
    .await;
}

async fn set_unavailable(app: &AppHandle) {
    let (executable, memory_access) = {
        let state = app.state::<AppState>();
        let process = state
            .process
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (
            process.resolved_executable(),
            process.memory_access_status(),
        )
    };
    update_snapshot(app, |snapshot| {
        snapshot.tosu.connection = TosuConnection::Unavailable;
        snapshot.tosu.executable_available = executable.is_some();
        snapshot.tosu.executable_path = executable
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned());
        snapshot.tosu.memory_access = memory_access;
        snapshot.tosu.message = Some(if executable.is_some() {
            "tosu is not running".into()
        } else {
            "tosu is not running and no executable has been selected or found on PATH".into()
        });
        clear_displayed_osu_state(snapshot);
    })
    .await;
}

async fn mark_disconnected(app: &AppHandle) {
    set_inactive(
        app,
        TosuConnection::Searching,
        Some("the tosu connection was interrupted; retrying".into()),
    )
    .await;
}

async fn set_inactive(app: &AppHandle, connection: TosuConnection, message: Option<String>) {
    update_snapshot(app, |snapshot| {
        snapshot.tosu.connection = connection;
        snapshot.tosu.message = message;
        clear_displayed_osu_state(snapshot);
    })
    .await;
}

fn clear_displayed_osu_state(snapshot: &mut crate::app_state::CompanionSnapshot) {
    snapshot.osu.running = false;
    snapshot.osu.state = OsuState::Unknown;
    snapshot.osu.client = None;
    snapshot.now_playing = None;
    snapshot.live_play = None;
}

async fn mark_memory_access_possibly_required(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    if current_snapshot(app).await.tosu.memory_access != TosuMemoryAccess::PossiblyRequired {
        update_snapshot(app, |snapshot| {
            if snapshot.tosu.memory_access != TosuMemoryAccess::Available {
                snapshot.tosu.memory_access = TosuMemoryAccess::PossiblyRequired;
                snapshot.tosu.message =
                    Some("tosu reported a Linux memory-access problem while reading osu!".into());
            }
        })
        .await;
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app;
}

fn indicates_memory_access_failure(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("failed to read address")
        || message.contains("cap_sys_ptrace")
        || message.contains("ptrace") && message.contains("permission")
}

async fn wait_or_tracking_change(
    tracking: &mut tokio::sync::watch::Receiver<bool>,
    delay: Duration,
) -> bool {
    tokio::select! {
        changed = tracking.changed() => changed.is_ok(),
        _ = tokio::time::sleep(delay) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_state_timeout_is_distinct_from_a_stale_transport() {
        let opened = Instant::now();
        let mut health = ConnectionHealth::new(opened);
        health.record_valid_state(opened + Duration::from_secs(1));
        health.record_activity(opened + Duration::from_secs(9));
        assert!(!health.transport_is_stale(opened + Duration::from_secs(12)));
        assert!(health.valid_state_timed_out(opened + Duration::from_secs(12)));
    }

    #[test]
    fn websocket_activity_keeps_an_idle_tosu_transport_healthy() {
        let opened = Instant::now();
        let mut health = ConnectionHealth::new(opened);
        health.record_activity(opened + Duration::from_secs(9));
        assert!(!health.transport_is_stale(opened + MAX_VALID_STATE_SILENCE));
        assert!(health.valid_state_timed_out(opened + MAX_VALID_STATE_SILENCE));
        assert!(health.transport_is_stale(opened + Duration::from_secs(24)));
    }
}
