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
};

use super::model::{PayloadError, RawTosuPayload};

const TOSU_SOCKET: &str = "ws://127.0.0.1:24050/websocket/v2";
const TOSU_ADDRESS: &str = "127.0.0.1:24050";
const UI_INTERVAL: Duration = Duration::from_millis(125);
const MAX_BACKOFF: Duration = Duration::from_secs(8);

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
            if current_snapshot(&app).await.tosu.connection != TosuConnection::Disabled {
                update_snapshot(&app, |snapshot| {
                    snapshot.tracking_enabled = false;
                    snapshot.tosu.connection = TosuConnection::Disabled;
                    snapshot.tosu.message = None;
                    snapshot.osu.running = false;
                    snapshot.osu.state = OsuState::Unknown;
                    snapshot.live_play = None;
                })
                .await;
            }
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

        let probe = tokio::time::timeout(
            Duration::from_millis(700),
            tokio::net::TcpStream::connect(TOSU_ADDRESS),
        )
        .await;

        if !matches!(probe, Ok(Ok(_))) {
            set_unavailable(&app).await;
            if wait_or_tracking_change(&mut tracking, backoff).await {
                continue;
            }
            backoff = (backoff * 2).min(MAX_BACKOFF);
            continue;
        }

        set_connection(&app, TosuConnection::Connecting, None).await;
        match tokio::time::timeout(Duration::from_secs(3), connect_async(TOSU_SOCKET)).await {
            Ok(Ok((socket, _))) => {
                backoff = Duration::from_millis(500);
                set_connection(&app, TosuConnection::Connected, None).await;
                monitor_socket(&app, socket, &mut tracking, &mut activity).await;
            }
            Ok(Err(error)) => {
                eprintln!("tosu WebSocket connection failed: {error}");
                set_connection(
                    &app,
                    TosuConnection::Error,
                    Some("tosu accepted a connection but its v2 WebSocket was unavailable".into()),
                )
                .await;
            }
            Err(_) => {
                set_connection(
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
        mark_disconnected(&app).await;
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
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut writer, mut reader) = socket.split();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut process_poll = tokio::time::interval(Duration::from_secs(1));
    process_poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_ui_emit = Instant::now() - UI_INTERVAL;
    let mut last_parse_log: Option<Instant> = None;
    let mut consecutive_unexpected = 0_u8;

    loop {
        tokio::select! {
            changed = tracking.changed() => {
                if changed.is_err() || !*tracking.borrow() {
                    let _ = writer.send(Message::Close(None)).await;
                    break;
                }
            }
            _ = heartbeat.tick() => {
                if writer.send(Message::Ping(Vec::new())).await.is_err() {
                    break;
                }
            }
            _ = process_poll.tick() => {
                refresh_process_state(app).await;
            }
            message = reader.next() => {
                match message {
                    Some(Ok(Message::Text(payload))) => {
                        match RawTosuPayload::parse(&payload) {
                            Ok(raw) => {
                                consecutive_unexpected = 0;
                                let normalized = raw.normalize();
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
                            Err(PayloadError::NotReady) => {
                                {
                                    let state = app.state::<AppState>();
                                    let mut snapshot = state.snapshot.write().await;
                                    snapshot.osu.running = false;
                                    snapshot.osu.state = OsuState::Unknown;
                                    snapshot.live_play = None;
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
                                if consecutive_unexpected == 3 {
                                    set_connection(
                                        app,
                                        TosuConnection::Error,
                                        Some("tosu returned an unexpected v2 payload".into()),
                                    ).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

async fn emit_current(app: &AppHandle) {
    use tauri::Emitter;

    let snapshot = current_snapshot(app).await;
    let _ = app.emit(SNAPSHOT_EVENT, &snapshot);
    crate::tray::sync_menu(app, &snapshot);
}

async fn refresh_process_state(app: &AppHandle) {
    let (owned, executable_available) = {
        let state = app.state::<AppState>();
        let mut process = state
            .process
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (
            process.is_owned(),
            super::process::TosuProcess::resolve_executable().is_some(),
        )
    };
    let current = current_snapshot(app).await;
    if current.tosu.process_owned != owned
        || current.tosu.executable_available != executable_available
    {
        update_snapshot(app, |snapshot| {
            snapshot.tosu.process_owned = owned;
            snapshot.tosu.executable_available = executable_available;
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

async fn set_unavailable(app: &AppHandle) {
    let executable_available = super::process::TosuProcess::resolve_executable().is_some();
    let memory_access = super::process::TosuProcess::memory_access_status();
    update_snapshot(app, |snapshot| {
        snapshot.tosu.connection = TosuConnection::Unavailable;
        snapshot.tosu.executable_available = executable_available;
        snapshot.tosu.memory_access = memory_access;
        snapshot.tosu.message = Some(if executable_available {
            "tosu is not running".into()
        } else {
            "tosu is not running and was not found on PATH".into()
        });
        snapshot.osu.running = false;
        snapshot.osu.state = OsuState::Unknown;
        snapshot.live_play = None;
    })
    .await;
}

async fn mark_disconnected(app: &AppHandle) {
    update_snapshot(app, |snapshot| {
        snapshot.tosu.connection = TosuConnection::Searching;
        snapshot.tosu.message = Some("the tosu connection was interrupted; retrying".into());
        snapshot.osu.running = false;
        snapshot.osu.state = OsuState::Unknown;
        snapshot.live_play = None;
    })
    .await;
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
