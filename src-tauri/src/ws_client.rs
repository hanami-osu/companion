use futures_util::StreamExt;
use serde_json::Value;
use tauri::AppHandle;
use tokio_tungstenite::connect_async;
use tauri::Emitter;

pub fn start_tosu_listener(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let url = "ws://127.0.0.1:24050/ws";
        
        loop {
            // Attempt to connect
            if let Ok((ws_stream, _)) = connect_async(url).await {
                println!("Connected to tosu WebSocket");
                let _ = app.emit("activity-log", "Connected to tosu WebSocket.");

                let (mut _write, mut read) = ws_stream.split();
                let mut last_state = String::new();

                while let Some(msg) = read.next().await {
                    if let Ok(msg) = msg {
                        if msg.is_text() {
                            let text = msg.to_text().unwrap();
                            if let Ok(json) = serde_json::from_str::<Value>(text) {
                                // Basic state transition detection based on common gosumemory/tosu schema
                                // The state is usually under menu.state
                                if let Some(state) = json.pointer("/menu/state").and_then(|s| s.as_u64()) {
                                    // 2 = Playing, 7 = Results Screen (Ranking)
                                    let current_state = match state {
                                        2 => "Playing",
                                        7 => "Ranking",
                                        _ => "Idle",
                                    };

                                    if current_state != last_state {
                                        if last_state == "Playing" && current_state == "Ranking" {
                                            println!("Play finished! Capturing score...");
                                            let _ = app.emit("activity-log", "Play finished! Capturing score...");
                                            
                                            // TODO: Extract score, UR, mods, md5 and send to Hanami Web
                                        }
                                        last_state = current_state.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
                println!("Disconnected from tosu WebSocket");
                let _ = app.emit("activity-log", "Disconnected from tosu WebSocket.");
            }

            // Retry after 5 seconds
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}
