use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::time::{Instant, MissedTickBehavior};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};

const TOSU_URL: &str = "ws://127.0.0.1:24050/ws";
const CONNECTION_EVENT: &str = "tosu-connection-status";
const RETRY_DELAY: Duration = Duration::from_secs(2);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(3);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Default)]
pub struct TosuConnectionState {
    connected: AtomicBool,
}

impl TosuConnectionState {
    fn get(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn set(&self, connected: bool) -> bool {
        self.connected.swap(connected, Ordering::SeqCst) != connected
    }
}

#[tauri::command]
pub fn is_tosu_connected(state: State<'_, TosuConnectionState>) -> bool {
    state.get()
}

fn publish_connection_status(app: &AppHandle, connected: bool) {
    let state = app.state::<TosuConnectionState>();

    if state.set(connected) {
        let _ = app.emit(CONNECTION_EVENT, connected);
    }
}

async fn monitor_connection<S>(app: &AppHandle, socket: WebSocketStream<S>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut write, mut read) = socket.split();
    let started_at = Instant::now();
    let mut last_verified = None;
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                let last_response = last_verified.unwrap_or(started_at);

                if last_response.elapsed() >= CONNECTION_TIMEOUT {
                    break;
                }

                if write.send(Message::Ping(Vec::new())).await.is_err() {
                    break;
                }
            }
            message = read.next() => {
                match message {
                    Some(Ok(Message::Pong(_))) => {
                        last_verified = Some(Instant::now());
                        publish_connection_status(app, true);
                    }
                    Some(Ok(Message::Text(payload))) if serde_json::from_str::<serde_json::Value>(&payload).is_ok() => {
                        last_verified = Some(Instant::now());
                        publish_connection_status(app, true);
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

pub fn start_tosu_listener(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            publish_connection_status(&app, false);

            if let Ok((socket, _)) = connect_async(TOSU_URL).await {
                monitor_connection(&app, socket).await;
            }

            publish_connection_status(&app, false);
            tokio::time::sleep(RETRY_DELAY).await;
        }
    });
}
