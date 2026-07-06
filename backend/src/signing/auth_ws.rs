use axum::{
    extract::{ws::Message, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use tracing::{error, info};

use super::AppState;

/// Handler for WebSocket connections for authentication sync.
pub async fn handle_auth_websocket(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!("[WS_AUTH] Received WebSocket upgrade request");
    ws.on_upgrade(|socket| async move {
        let (mut write, mut read) = socket.split();
        info!("[WS_AUTH] WebSocket connection established");

        // Authentication check using JWT or token
        let mut is_authenticated = false;
        let mut client_id = String::new();
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Check for authentication token
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(token) = json.get("token").and_then(|t| t.as_str()) {
                            // Verify the JWT cryptographically — the identity is the
                            // signed `sub` claim, never the raw token bytes.
                            match state.jwt.verify(token) {
                                Ok(claims) => {
                                    is_authenticated = true;
                                    client_id = claims.sub;
                                    info!(
                                        "[WS_AUTH] Client {} authenticated successfully",
                                        client_id
                                    );
                                    break;
                                }
                                Err(_) => {
                                    error!("[WS_AUTH] Invalid or expired token received");
                                    let _ = write
                                        .send(Message::Text("Invalid token".to_string().into()))
                                        .await;
                                    let _ = write.close().await;
                                    return;
                                }
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("[WS_AUTH] Client closed connection during auth");
                    return;
                }
                Err(e) => {
                    error!("[WS_AUTH] Error during auth handshake: {}", e);
                    let _ = write.close().await;
                    return;
                }
                _ => {}
            }
        }

        if !is_authenticated {
            error!("[WS_AUTH] Unauthorized WebSocket connection attempt");
            let _ = write
                .send(Message::Text("Unauthorized".to_string().into()))
                .await;
            let _ = write.close().await;
            return;
        }

        // Send initial authentication data
        let initial_data = serde_json::json!({
            "status": "connected",
            "message": "Authenticated successfully",
            "client_id": client_id
        })
        .to_string();
        if let Err(e) = write.send(Message::Text(initial_data.into())).await {
            error!("[WS_AUTH] Failed to send initial data: {}", e);
            return;
        }

        // Main loop to handle incoming messages and send updates
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    info!("[WS_AUTH] Received message from {}: {}", client_id, text);
                    // Handle client requests for auth data
                    let response = serde_json::json!({
                        "login_status": true,
                        "token": "updated_token",
                        "wallet_pubkey": "updated_pubkey"
                    })
                    .to_string();
                    if let Err(e) = write.send(Message::Text(response.into())).await {
                        error!("[WS_AUTH] Failed to send response to {}: {}", client_id, e);
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("[WS_AUTH] Client {} closed connection", client_id);
                    break;
                }
                Err(e) => {
                    error!("[WS_AUTH] WebSocket error for client {}: {}", client_id, e);
                    break;
                }
                _ => {}
            }
        }

        info!(
            "[WS_AUTH] WebSocket connection closed for client {}",
            client_id
        );
    })
}
