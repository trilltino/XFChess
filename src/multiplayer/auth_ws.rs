use bevy::prelude::*;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use std::env;
use tracing::{error, info};
use serde_json;

/// Resource for managing WebSocket connection to backend for authentication sync
#[derive(Resource)]
pub struct AuthWebSocket {
    pub tx: Option<mpsc::Sender<String>>,
    pub rx: Option<mpsc::Receiver<String>>,
    pub connected: bool,
    pub auth_data: Option<String>,
}

impl Default for AuthWebSocket {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            tx: Some(tx),
            rx: Some(rx),
            connected: false,
            auth_data: None,
        }
    }
}

/// Plugin for WebSocket authentication sync
pub struct AuthWebSocketPlugin;

impl Plugin for AuthWebSocketPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AuthWebSocket>()
            .add_systems(Startup, start_auth_websocket)
            .add_systems(Update, handle_auth_messages);
    }
}

/// Start WebSocket connection to backend for authentication sync
fn start_auth_websocket(mut auth_ws: ResMut<AuthWebSocket>) {
    let backend_url = env::var("BACKEND_URL").unwrap_or_else(|_| "ws://localhost:8090".to_string());
    let ws_url = if backend_url.starts_with("http") {
        backend_url.replace("http", "ws") + "/ws/auth"
    } else {
        format!("{}/ws/auth", backend_url)
    };
    info!("Starting WebSocket connection to {}", ws_url);

    let tx = auth_ws.tx.take().unwrap();
    bevy::tasks::IoTaskPool::get().spawn(async move {
        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                info!("WebSocket connection to backend established");
                let (mut write, mut read) = ws_stream.split();

                // Send authentication token if available
                let auth_token = env::var("XFCHESS_AUTH_TOKEN").unwrap_or_else(|_| "placeholder_token".to_string());
                let initial_data = serde_json::json!({
                    "token": auth_token
                }).to_string();
                if let Err(e) = write.send(Message::Text(initial_data)).await {
                    error!("Failed to send auth token: {}", e);
                    return;
                }

                // Main loop for sending and receiving messages
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            info!("Received auth data: {}", text);
                            if let Err(e) = tx.send(text).await {
                                error!("Failed to send auth data to game: {}", e);
                                break;
                            }
                        }
                        Ok(Message::Close(_)) => {
                            info!("WebSocket connection closed by server");
                            break;
                        }
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("Failed to connect to WebSocket: {}", e);
            }
        }
    }).detach();
}

/// Handle incoming authentication messages from WebSocket
fn handle_auth_messages(mut auth_ws: ResMut<AuthWebSocket>) {
    if let Some(ref mut rx) = auth_ws.rx {
        while let Ok(msg) = rx.try_recv() {
            auth_ws.auth_data = Some(msg);
            auth_ws.connected = true;
            info!("Updated auth data from WebSocket");
        }
    }
}
