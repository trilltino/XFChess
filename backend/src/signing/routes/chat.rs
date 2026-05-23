//! In-game P2P chat relay over Braid-HTTP 209.
//!
//! Routes:
//!   GET  /game/:id/chat          — Braid-209 subscription stream (or plain JSON snapshot)
//!   PUT  /game/:id/chat          — publish a new chat message; broadcasts to all subscribers
//!
//! Each game has its own `broadcast::Sender<BraidUpdate>`. Capacity is 64 — messages are
//! ephemeral (not persisted between server restarts), matching the existing P2P relay design.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, put},
    Json, Router,
};
use bytes::Bytes;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, info, warn};
use xfchess_braid_server::resource::protocol::{format_chunk, format_heartbeat, BraidUpdate};

use crate::signing::AppState;

// ── Shared state ──────────────────────────────────────────────────────────────

const CHAT_BROADCAST_CAP: usize = 64;
const HEARTBEAT_SECS: u64 = 20;
const BRAID_BOUNDARY: &str = "xfchess-chat";
const MAX_MESSAGE_LEN: usize = 500;

pub type ChatRelayState = Arc<RwLock<HashMap<String, broadcast::Sender<BraidUpdate>>>>;

pub fn new_chat_relay() -> ChatRelayState {
    Arc::new(RwLock::new(HashMap::new()))
}

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatMessageReq {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
}

// ── Routes ────────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/game/:game_id/chat", get(get_chat).put(put_chat))
}

// ── GET /game/:game_id/chat ───────────────────────────────────────────────────

async fn get_chat(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if wants_subscribe(&headers) {
        subscribe_stream(state.chat_relay, game_id).await
    } else {
        // Plain GET: return empty JSON array (no history persistence).
        (StatusCode::OK, Json(serde_json::json!([]))).into_response()
    }
}

fn wants_subscribe(headers: &HeaderMap) -> bool {
    headers
        .get("Prefer")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("subscribe"))
        .unwrap_or(false)
        || headers
            .get("Subscribe")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("keep-alive"))
            .unwrap_or(false)
}

async fn subscribe_stream(relay: ChatRelayState, game_id: String) -> Response {
    let rx = {
        let mut map = relay.write().await;
        let sender = map
            .entry(game_id.clone())
            .or_insert_with(|| broadcast::channel(CHAT_BROADCAST_CAP).0);
        sender.subscribe()
    };

    let boundary = BRAID_BOUNDARY;
    let ct = format!("multipart/mixed; boundary=\"{}\"", boundary);
    // Empty snapshot — chat has no prior history in this relay.
    let snapshot = BraidUpdate::snapshot(0, serde_json::json!([]));
    let snapshot_chunk = format_chunk(boundary, &snapshot);
    let hb_chunk = format_heartbeat(boundary);

    let rx_stream = BroadcastStream::new(rx);

    let stream = async_stream::stream! {
        debug!("[chat] subscriber connected to game {}", game_id);
        yield Ok::<Bytes, String>(snapshot_chunk);

        let mut ticker = interval(Duration::from_secs(HEARTBEAT_SECS));
        ticker.tick().await;

        tokio::pin!(rx_stream);
        loop {
            tokio::select! {
                maybe_update = rx_stream.next() => {
                    match maybe_update {
                        Some(Ok(update)) => yield Ok(format_chunk(boundary, &update)),
                        Some(Err(e)) => warn!("[chat] broadcast lag for game: {}", e),
                        None => break,
                    }
                }
                _ = ticker.tick() => {
                    yield Ok(hb_chunk.clone());
                }
            }
        }
        debug!("[chat] subscriber disconnected");
    };

    Response::builder()
        .status(209)
        .header("Content-Type", ct)
        .header("Cache-Control", "no-cache")
        .header("Transfer-Encoding", "chunked")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// ── PUT /game/:game_id/chat ───────────────────────────────────────────────────

async fn put_chat(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
    Json(req): Json<ChatMessageReq>,
) -> StatusCode {
    let text = req.text.trim().to_string();
    if text.is_empty() || text.len() > MAX_MESSAGE_LEN {
        return StatusCode::UNPROCESSABLE_ENTITY;
    }

    let body = serde_json::json!({
        "type": "chat",
        "player": req.player,
        "text": text,
        "timestamp_ms": req.timestamp_ms,
    });

    let update = {
        let map = state.chat_relay.read().await;
        if let Some(sender) = map.get(&game_id) {
            let version = req.timestamp_ms;
            let update = BraidUpdate::snapshot(version, body);
            let _ = sender.send(update.clone());
            Some(update)
        } else {
            None
        }
    };

    if update.is_none() {
        // No subscribers yet — create the channel and send anyway so the sender
        // can subscribe after this PUT arrives.
        let mut map = state.chat_relay.write().await;
        let sender = map
            .entry(game_id.clone())
            .or_insert_with(|| broadcast::channel(CHAT_BROADCAST_CAP).0);
        let update = BraidUpdate::snapshot(req.timestamp_ms, serde_json::json!({
            "type": "chat",
            "player": req.player,
            "text": text,
            "timestamp_ms": req.timestamp_ms,
        }));
        let _ = sender.send(update);
    }

    info!("[chat] {} → game {}: {:?}", req.player, game_id, &text[..text.len().min(40)]);
    StatusCode::OK
}
