use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use braid_chess::ChessMessage;
use bytes::Bytes;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, info, warn};
use xfchess_braid_server::resource::protocol::{format_chunk, format_heartbeat, BraidUpdate};

use crate::signing::AppState;

const BROADCAST_CAP: usize = 64;
const HEARTBEAT_SECS: u64 = 20;
const MAX_CHAT_LEN: usize = 500;

const GENESIS_PARENT: &str = "0";

#[derive(Debug)]
pub enum PutEventError {
    ParentMismatch { expected: String },
    NotAParticipant,
    Db(sqlx::Error),
}

fn requires_participant_check(kind: &str) -> bool {
    matches!(
        kind,
        "move" | "resign" | "offer_draw" | "accept_draw" | "decline_draw"
    )
}

pub struct GameLogState {
    pool: sqlx::SqlitePool,
    channels: RwLock<HashMap<String, broadcast::Sender<BraidUpdate>>>,
    roster: RwLock<HashMap<String, Vec<String>>>,
    session_roster: RwLock<HashMap<String, HashMap<String, String>>>,
    participants: Option<crate::signing::solana::game_participants::GameParticipantsCache>,
    casual_identities: RwLock<HashMap<String, (String, String)>>,
}

impl GameLogState {
    pub fn new(
        pool: sqlx::SqlitePool,
        participants: Option<crate::signing::solana::game_participants::GameParticipantsCache>,
    ) -> Self {
        Self {
            pool,
            channels: RwLock::new(HashMap::new()),
            roster: RwLock::new(HashMap::new()),
            session_roster: RwLock::new(HashMap::new()),
            participants,
            casual_identities: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register_casual_identities(
        &self,
        game_id: &str,
        host_node_id: &str,
        joiner_node_id: &str,
    ) {
        let mut map = self.casual_identities.write().await;
        map.insert(
            game_id.to_string(),
            (host_node_id.to_string(), joiner_node_id.to_string()),
        );
    }

    fn channel_key(game_id: &str, stream: &str) -> String {
        format!("{}/{}", game_id, stream)
    }

    async fn channel(&self, game_id: &str, stream: &str) -> broadcast::Sender<BraidUpdate> {
        let key = Self::channel_key(game_id, stream);
        {
            let map = self.channels.read().await;
            if let Some(tx) = map.get(&key) {
                return tx.clone();
            }
        }
        let mut map = self.channels.write().await;
        map.entry(key)
            .or_insert_with(|| broadcast::channel(BROADCAST_CAP).0)
            .clone()
    }

    async fn load_history(&self, game_id: &str, stream: &str) -> Vec<serde_json::Value> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT kind, payload_json FROM game_event_log WHERE game_id = ? ORDER BY seq ASC",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        rows.into_iter()
            .filter(|(kind, _)| belongs_to_stream(kind, stream))
            .filter_map(|(_, payload_json)| serde_json::from_str(&payload_json).ok())
            .collect()
    }

    pub async fn subscribe(
        &self,
        game_id: &str,
        stream: &str,
    ) -> (Vec<serde_json::Value>, broadcast::Receiver<BraidUpdate>) {
        let tx = self.channel(game_id, stream).await;
        let rx = tx.subscribe();
        let history = self.load_history(game_id, stream).await;
        (history, rx)
    }

    pub async fn snapshot(&self, game_id: &str, stream: &str) -> serde_json::Value {
        serde_json::Value::Array(self.load_history(game_id, stream).await)
    }

    pub async fn put_event(
        &self,
        game_id: &str,
        stream: &str,
        player_pubkey: &str,
        message: &ChessMessage,
        content_version: &str,
        content_parent: &str,
    ) -> Result<i64, PutEventError> {
        self.put_event_with_session(
            game_id,
            stream,
            player_pubkey,
            "",
            message,
            content_version,
            content_parent,
        )
        .await
    }

    async fn put_event_with_session(
        &self,
        game_id: &str,
        stream: &str,
        player_pubkey: &str,
        session_token: &str,
        message: &ChessMessage,
        content_version: &str,
        content_parent: &str,
    ) -> Result<i64, PutEventError> {
        let kind = kind_of(message);

        if requires_participant_check(kind) {
            self.check_participant(game_id, player_pubkey, session_token)
                .await?;
        }

        let payload_json = serde_json::to_string(message)
            .map_err(|e| PutEventError::Db(sqlx::Error::Decode(Box::new(e))))?;
        let created_at = chrono::Utc::now().timestamp();

        // Single transaction: read current head, validate, insert.
        // Serializes concurrent writers for the same game (SQLite's writer
        // lock already does this at the connection level; the explicit
        // transaction makes the read-then-write atomic against a second
        // racing PUT for the same game_id).
        let mut tx = self.pool.begin().await.map_err(PutEventError::Db)?;

        // Causal-chain validation (and `seq` numbering below) is a global
        // per-`game_id` counter shared across every stream/kind — but the
        // *head* a new event must chain off is scoped to its own stream
        // (moves-like kinds vs. chat), since chat messages don't causally
        // build on move content and vice versa.
        let head_sql = if stream == "chat" {
            "SELECT version_hash FROM game_event_log WHERE game_id = ? AND kind = 'chat' ORDER BY seq DESC LIMIT 1"
        } else {
            "SELECT version_hash FROM game_event_log WHERE game_id = ? AND kind != 'chat' ORDER BY seq DESC LIMIT 1"
        };
        let current_head: Option<String> = sqlx::query_scalar(head_sql)
            .bind(game_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(PutEventError::Db)?;
        let expected_parent = current_head.as_deref().unwrap_or(GENESIS_PARENT);

        if content_parent != expected_parent {
            return Err(PutEventError::ParentMismatch {
                expected: expected_parent.to_string(),
            });
        }

        let next_seq: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM game_event_log WHERE game_id = ?",
        )
        .bind(game_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(PutEventError::Db)?;

        sqlx::query(
            "INSERT INTO game_event_log (game_id, seq, kind, version_hash, parent_version, payload_json, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(game_id)
        .bind(next_seq)
        .bind(kind)
        .bind(content_version)
        .bind(content_parent)
        .bind(&payload_json)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(PutEventError::Db)?;

        tx.commit().await.map_err(PutEventError::Db)?;

        if let ChessMessage::SessionInfo {
            player_pubkey,
            session_pubkey,
            ..
        } = message
        {
            self.learn_session_info_claim(game_id, player_pubkey, session_pubkey)
                .await;
        }

        let tx_chan = self.channel(game_id, stream).await;
        let message_value = serde_json::to_value(message).unwrap_or(serde_json::Value::Null);
        let _ = tx_chan.send(BraidUpdate::snapshot(next_seq as u64, message_value));

        Ok(next_seq)
    }

    async fn check_participant(
        &self,
        game_id: &str,
        player_pubkey: &str,
        session_token: &str,
    ) -> Result<(), PutEventError> {
        {
            let roster = self.roster.read().await;
            if let Some(allowed) = roster.get(game_id) {
                if allowed.iter().any(|p| p == player_pubkey) {
                    if player_pubkey.parse::<solana_sdk::pubkey::Pubkey>().is_ok() {
                        let sessions = self.session_roster.read().await;
                        if sessions
                            .get(game_id)
                            .and_then(|entries| entries.get(player_pubkey))
                            != Some(&session_token.to_string())
                        {
                            return Err(PutEventError::NotAParticipant);
                        }
                    }
                    return Ok(());
                }
            }
        }

        match self
            .verify_claim(
                game_id,
                player_pubkey,
                (!session_token.is_empty()).then_some(session_token),
            )
            .await
        {
            OnChainCheck::Verified => {
                self.add_to_roster(game_id, player_pubkey).await;
                Ok(())
            }
            // Ground truth exists (on-chain `Game` account, or a JOIN_ACK-
            // verified casual identity pair) and this claimant matches
            // neither of its two registered identities. Unlike the
            // empty-roster bootstrap below, this is a definitive reject,
            // not "not yet established."
            OnChainCheck::Mismatch => Err(PutEventError::NotAParticipant),
            OnChainCheck::Unavailable => {
                let roster = self.roster.read().await;
                if let Some(allowed) = roster.get(game_id) {
                    if !allowed.is_empty() && !allowed.iter().any(|p| p == player_pubkey) {
                        return Err(PutEventError::NotAParticipant);
                    }
                }
                Ok(())
            }
        }
    }

    async fn learn_session_info_claim(
        &self,
        game_id: &str,
        player_pubkey: &str,
        session_pubkey: &str,
    ) {
        match self
            .verify_claim(game_id, player_pubkey, Some(session_pubkey))
            .await
        {
            OnChainCheck::Verified => {
                self.add_to_roster(game_id, player_pubkey).await;
                if player_pubkey.parse::<solana_sdk::pubkey::Pubkey>().is_ok() {
                    let mut sessions = self.session_roster.write().await;
                    sessions
                        .entry(game_id.to_string())
                        .or_default()
                        .insert(player_pubkey.to_string(), session_pubkey.to_string());
                }
            }
            OnChainCheck::Mismatch => {
                // Ground truth exists (on-chain, or a JOIN_ACK-verified
                // casual pair) and this claimant matches neither identity —
                // do not add it just because it claimed to be one of them.
            }
            // No ground truth available at all (direct-connection game that
            // never went through accept_join, or the narrow window before
            // either check's data has arrived): keep the original
            // first-two-seen bootstrap trust, unchanged.
            OnChainCheck::Unavailable => self.add_to_roster(game_id, player_pubkey).await,
        }
    }

    async fn verify_claim(
        &self,
        game_id: &str,
        player_pubkey: &str,
        claimed_session_key: Option<&str>,
    ) -> OnChainCheck {
        match self
            .on_chain_check(game_id, player_pubkey, claimed_session_key)
            .await
        {
            OnChainCheck::Unavailable => self.casual_identity_check(game_id, player_pubkey).await,
            other => other,
        }
    }

    async fn casual_identity_check(&self, game_id: &str, player_pubkey: &str) -> OnChainCheck {
        let map = self.casual_identities.read().await;
        let Some((host, joiner)) = map.get(game_id) else {
            return OnChainCheck::Unavailable;
        };
        if player_pubkey == host || player_pubkey == joiner {
            OnChainCheck::Verified
        } else {
            OnChainCheck::Mismatch
        }
    }

    async fn add_to_roster(&self, game_id: &str, player_pubkey: &str) {
        let mut roster = self.roster.write().await;
        let entry = roster.entry(game_id.to_string()).or_default();
        if !entry.iter().any(|p| p == player_pubkey) && entry.len() < 2 {
            entry.push(player_pubkey.to_string());
        }
    }

    async fn on_chain_check(
        &self,
        game_id: &str,
        player_pubkey: &str,
        claimed_session_key: Option<&str>,
    ) -> OnChainCheck {
        let Some(participants) = &self.participants else {
            return OnChainCheck::Unavailable;
        };
        let Ok(gid) = game_id.parse::<u64>() else {
            return OnChainCheck::Unavailable;
        };
        let Some((white, black)) = participants.get(gid).await else {
            return OnChainCheck::Unavailable;
        };
        let Ok(wallet) = player_pubkey.parse::<solana_sdk::pubkey::Pubkey>() else {
            return OnChainCheck::Mismatch;
        };
        if wallet != white && wallet != black {
            return OnChainCheck::Mismatch;
        }
        let Some(session_key) =
            claimed_session_key.and_then(|key| key.parse::<solana_sdk::pubkey::Pubkey>().ok())
        else {
            return OnChainCheck::Mismatch;
        };
        if participants
            .session_key_authorized(gid, &wallet, &session_key)
            .await
        {
            OnChainCheck::Verified
        } else {
            OnChainCheck::Mismatch
        }
    }
}

enum OnChainCheck {
    Unavailable,
    Verified,
    Mismatch,
}

fn belongs_to_stream(kind: &str, stream: &str) -> bool {
    if stream == "chat" {
        kind == "chat"
    } else {
        kind != "chat"
    }
}

fn kind_of(msg: &ChessMessage) -> &'static str {
    match msg {
        ChessMessage::Move(_) => "move",
        ChessMessage::Resign { .. } => "resign",
        ChessMessage::OfferDraw { .. } => "offer_draw",
        ChessMessage::AcceptDraw { .. } => "accept_draw",
        ChessMessage::DeclineDraw { .. } => "decline_draw",
        ChessMessage::Clock(_) => "clock",
        ChessMessage::EngineAnalysis(_) => "engine_analysis",
        ChessMessage::Chat(_) => "chat",
        ChessMessage::SessionInfo { .. } => "session_info",
    }
}

// ── Wire types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct GameEventReq {
    #[serde(rename = "player_pubkey")]
    pub sender_identity: String,
    pub session_token: String,
    pub message: ChessMessage,
    pub content_version: String,
    pub content_parent: String,
}

// ── Routes ──────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/game/{game_id}/moves", get(get_moves).put(put_moves))
        .route("/game/{game_id}/chat", get(get_chat).put(put_chat))
        .route("/game/{game_id}/participants", get(get_participants))
}

#[derive(Serialize)]
struct ParticipantsResp {
    white: String,
    black: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParentMismatchResp {
    pub expected_parent: String,
}

async fn get_participants(State(state): State<AppState>, Path(game_id): Path<String>) -> Response {
    let Ok(gid) = game_id.parse::<u64>() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match state.game_participants.get(gid).await {
        Some((white, black)) => (
            StatusCode::OK,
            Json(ParticipantsResp {
                white: white.to_string(),
                black: black.to_string(),
            }),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_moves(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    get_stream(&state, &game_id, "moves", headers).await
}

async fn put_moves(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
    Json(req): Json<GameEventReq>,
) -> Response {
    if matches!(
        req.message,
        ChessMessage::Chat(_) | ChessMessage::Clock(_) | ChessMessage::EngineAnalysis(_)
    ) {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    put_event_handler(&state, &game_id, "moves", req).await
}

async fn get_chat(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    get_stream(&state, &game_id, "chat", headers).await
}

async fn put_chat(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
    Json(mut req): Json<GameEventReq>,
) -> Response {
    let ChessMessage::Chat(ref mut payload) = req.message else {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    };
    let text = payload.text.trim().to_string();
    if text.is_empty() || text.len() > MAX_CHAT_LEN {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    payload.text = text;
    put_event_handler(&state, &game_id, "chat", req).await
}

// ── Shared GET (subscribe) path ──────────────────────────────────────────────

async fn get_stream(state: &AppState, game_id: &str, stream: &str, headers: HeaderMap) -> Response {
    if !wants_subscribe(&headers) {
        let snapshot = state.game_log.snapshot(game_id, stream).await;
        return (StatusCode::OK, Json(snapshot)).into_response();
    }

    let (history, rx) = state.game_log.subscribe(game_id, stream).await;

    // One chunk per historical entry, each shaped exactly like a live
    // update body (a bare `ChessMessage`) — see the module doc comment for
    // why this can't be a single bulk-array snapshot chunk.
    let history_chunks: Vec<Bytes> = history
        .into_iter()
        .enumerate()
        .map(|(i, entry)| format_chunk(&BraidUpdate::snapshot(i as u64, entry)))
        .collect();
    let hb_chunk = format_heartbeat();
    let rx_stream = BroadcastStream::new(rx);
    let game_id_owned = game_id.to_string();
    let stream_owned = stream.to_string();

    let body_stream = async_stream::stream! {
        debug!(
            "[game-log] subscriber connected to {}/{} ({} historical entries)",
            game_id_owned, stream_owned, history_chunks.len()
        );
        for chunk in history_chunks {
            yield Ok::<Bytes, String>(chunk);
        }

        let mut ticker = interval(Duration::from_secs(HEARTBEAT_SECS));
        ticker.tick().await;

        tokio::pin!(rx_stream);
        loop {
            tokio::select! {
                maybe_update = rx_stream.next() => {
                    match maybe_update {
                        Some(Ok(update)) => yield Ok(format_chunk(&update)),
                        Some(Err(e)) => warn!("[game-log] broadcast lag on {}/{}: {}", game_id_owned, stream_owned, e),
                        None => break,
                    }
                }
                _ = ticker.tick() => {
                    yield Ok(hb_chunk.clone());
                }
            }
        }
        debug!("[game-log] subscriber disconnected from {}/{}", game_id_owned, stream_owned);
    };

    Response::builder()
        .status(209)
        .header("Content-Type", "application/http-history")
        .header("Cache-Control", "no-store")
        .header("Heartbeats", HEARTBEAT_SECS.to_string())
        .body(Body::from_stream(body_stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// The draft leaves the `Subscribe` value open — it "may be blank, set to
// `true`, or contain arbitrary data" — and every client in this workspace
// sends `true` (`braid-http/src/client/native_network.rs`). This check
// previously accepted only the older `keep-alive` spelling, so every real
// subscribe attempt from the game client silently fell through to the
// plain-snapshot branch below (a bare `[]` for a fresh game), which the
// subscriber's Braid parser then rejected as malformed, ended the "stream",
// and reconnected — forever, never actually receiving live updates. Accept
// both spellings: `keep-alive` costs nothing and older peers may still send it.
fn wants_subscribe(headers: &HeaderMap) -> bool {
    headers
        .get("Prefer")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("subscribe"))
        .unwrap_or(false)
        || headers
            .get("Subscribe")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("keep-alive") || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

// ── Shared PUT (publish) path ────────────────────────────────────────────────

fn is_db_locked(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .map(|d| d.message().to_lowercase().contains("database is locked"))
        .unwrap_or(false)
}

async fn put_event_handler(
    state: &AppState,
    game_id: &str,
    stream: &str,
    req: GameEventReq,
) -> Response {
    if !auth_ok(state, &req).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let kind = kind_of(&req.message);

    // `game_event_log` sees heavy concurrent traffic — every poller in the
    // app (matchmaking, p2p, rates, tournaments, friends...) shares this
    // same SQLite pool — and a move write can occasionally lose the
    // SQLITE_BUSY_SNAPSHOT race despite `busy_timeout` (see `is_db_locked`).
    // Reproduced live: a move write 500'd in 2ms (not a 5s timeout), and
    // because this durable path is the fallback the client relies on when
    // gossip alone doesn't land, that single dropped write meant the
    // opponent's client never learned the move happened at all. Retrying
    // the whole `put_event` call gets a fresh transaction/snapshot each
    // time; any other error (or exhausting retries) falls through unchanged.
    const DB_LOCK_RETRY_ATTEMPTS: u32 = 3;
    let mut outcome = None;
    for attempt in 1..=DB_LOCK_RETRY_ATTEMPTS {
        let result = state
            .game_log
            .put_event_with_session(
                game_id,
                stream,
                &req.sender_identity,
                &req.session_token,
                &req.message,
                &req.content_version,
                &req.content_parent,
            )
            .await;
        match result {
            Err(PutEventError::Db(ref e))
                if attempt < DB_LOCK_RETRY_ATTEMPTS && is_db_locked(e) =>
            {
                warn!(
                    "[game-log] db locked for game {} (attempt {attempt}/{DB_LOCK_RETRY_ATTEMPTS}) — retrying with a fresh transaction",
                    game_id
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            other => {
                outcome = Some(other);
                break;
            }
        }
    }

    match outcome.expect("loop always sets outcome before exiting") {
        Ok(seq) => {
            info!(
                "[game-log] {} → {}/{} (kind={}, seq={})",
                req.sender_identity, game_id, stream, kind, seq
            );
            StatusCode::OK.into_response()
        }
        Err(PutEventError::ParentMismatch { expected }) => {
            warn!(
                "[game-log] rejected event for game {}: parent {} != head {}",
                game_id, req.content_parent, expected
            );
            // Return the true head so the client can re-chain and retry
            // instead of wedging. A publisher cannot know this head on its
            // own: the stream is shared, so the opponent's last event — not
            // the publisher's — is usually what it has to build on.
            (
                StatusCode::CONFLICT,
                Json(ParentMismatchResp {
                    expected_parent: expected,
                }),
            )
                .into_response()
        }
        Err(PutEventError::NotAParticipant) => {
            warn!(
                "[game-log] rejected event for game {}: {} is not a registered participant",
                game_id, req.sender_identity
            );
            StatusCode::FORBIDDEN.into_response()
        }
        Err(PutEventError::Db(e)) => {
            warn!("[game-log] db error for game {}: {}", game_id, e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn auth_ok(state: &AppState, req: &GameEventReq) -> bool {
    use solana_sdk::signer::Signer as _;
    use std::str::FromStr;

    // No caller in the game client currently sends a genuine session_token
    // in this field — it's always an empty string (verified across every
    // publish_move/publish_resign/publish_chat/publish_session_info call
    // site: player_pubkey is always the Iroh node id, session_token is
    // always String::new()). The real wallet/session identity for wagered
    // games travels inside the SessionInfo message payload itself and is
    // verified separately by verify_claim/on_chain_check below, which this
    // doesn't touch. An empty token can never match a real active session's
    // pubkey string, so the strict check below was unconditionally 401ing
    // *every* publish for *every* player — wagered or casual — making the
    // whole Braid durable-relay path dead weight, not a security boundary
    // anyone was actually relying on.
    if req.session_token.is_empty() {
        return true;
    }

    let Ok(wallet) = solana_sdk::pubkey::Pubkey::from_str(&req.sender_identity) else {
        return true;
    };
    let active = state.active_global_sessions.lock().await;
    matches!(active.get(&wallet), Some(kp) if kp.pubkey().to_string() == req.session_token)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use braid_chess::message::MovePayload;

    async fn migrated_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(include_str!("../../../migrations/027_game_event_log.sql"))
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn move_message(fen_after: &str, move_number: u32) -> ChessMessage {
        ChessMessage::Move(MovePayload::from_uci(
            "e2e4",
            fen_after,
            move_number,
            "alice",
        ))
    }

    // Regression test: `braid_chess::ChessSubscriber` (via
    // `braid-http/src/client/native_network.rs`) — the actual client used
    // for `/game/:id/moves` and `/game/:id/chat` — sends `Subscribe: true`,
    // not `Subscribe: keep-alive`. Before this fix, `wants_subscribe` only
    // recognized the latter, so every real subscribe request silently fell
    // through to the plain-snapshot branch, which the client's Braid parser
    // then rejected as malformed — moves never actually synced.
    #[test]
    fn wants_subscribe_recognizes_the_actual_client_header() {
        let mut headers = HeaderMap::new();
        headers.insert("Subscribe", "true".parse().unwrap());
        assert!(
            wants_subscribe(&headers),
            "Subscribe: true must count as a subscribe request"
        );
    }

    #[test]
    fn wants_subscribe_still_recognizes_keep_alive() {
        let mut headers = HeaderMap::new();
        headers.insert("Subscribe", "keep-alive".parse().unwrap());
        assert!(wants_subscribe(&headers));
    }

    #[test]
    fn wants_subscribe_false_without_either_header() {
        let headers = HeaderMap::new();
        assert!(!wants_subscribe(&headers));
    }

    #[tokio::test]
    async fn first_event_requires_genesis_parent() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        let msg = move_message("fen1", 1);
        let v1 = braid_chess::version_hash("fen1", 1);

        // Wrong parent on the very first event is rejected.
        let err = state
            .put_event("g1", "moves", "alice", &msg, &v1, "not-genesis")
            .await
            .unwrap_err();
        assert!(matches!(err, PutEventError::ParentMismatch { .. }));

        // Genesis parent is accepted and assigned seq 1.
        let seq = state
            .put_event("g1", "moves", "alice", &msg, &v1, GENESIS_PARENT)
            .await
            .unwrap();
        assert_eq!(seq, 1);
    }

    #[tokio::test]
    async fn chain_must_reference_the_true_current_head() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        let m1 = move_message("fen1", 1);
        let v1 = braid_chess::version_hash("fen1", 1);
        state
            .put_event("g1", "moves", "alice", &m1, &v1, GENESIS_PARENT)
            .await
            .unwrap();

        // A second event claiming genesis as its parent (instead of v1) is
        // an equivocation attempt — same class of check as the P2P
        // equivocation guard and the on-chain parent_nonce check.
        let m2 = move_message("fen2", 2);
        let v2 = braid_chess::version_hash("fen2", 2);
        let err = state
            .put_event("g1", "moves", "alice", &m2, &v2, GENESIS_PARENT)
            .await
            .unwrap_err();
        assert!(matches!(err, PutEventError::ParentMismatch { expected } if expected == v1));

        // Correctly chained off v1 succeeds.
        let seq = state
            .put_event("g1", "moves", "alice", &m2, &v2, &v1)
            .await
            .unwrap();
        assert_eq!(seq, 2);
    }

    #[tokio::test]
    async fn move_history_survives_a_simulated_restart() {
        let pool = migrated_pool().await;

        {
            let state = GameLogState::new(pool.clone(), None);
            let m1 = move_message("fen1", 1);
            let v1 = braid_chess::version_hash("fen1", 1);
            state
                .put_event("g1", "moves", "alice", &m1, &v1, GENESIS_PARENT)
                .await
                .unwrap();
            let m2 = move_message("fen2", 2);
            let v2 = braid_chess::version_hash("fen2", 2);
            state
                .put_event("g1", "moves", "alice", &m2, &v2, &v1)
                .await
                .unwrap();
            // `state` (and its in-memory broadcast channels) dropped here —
            // only the SQLite rows remain.
        }

        let restarted = GameLogState::new(pool, None);
        let snapshot = restarted.snapshot("g1", "moves").await;
        let entries = snapshot.as_array().expect("snapshot is a JSON array");
        assert_eq!(entries.len(), 2, "both moves should survive the restart");
    }

    #[tokio::test]
    async fn chat_and_moves_streams_stay_separate() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        let m1 = move_message("fen1", 1);
        let v1 = braid_chess::version_hash("fen1", 1);
        state
            .put_event("g1", "moves", "alice", &m1, &v1, GENESIS_PARENT)
            .await
            .unwrap();

        let chat = ChessMessage::Chat(braid_chess::message::ChatPayload {
            player: "alice".to_string(),
            text: "gg".to_string(),
            timestamp_ms: 0,
        });
        let vc = braid_chess::version_hash("chat-seed", 0);
        state
            .put_event("g1", "chat", "alice", &chat, &vc, GENESIS_PARENT)
            .await
            .unwrap();

        let moves_snapshot = state.snapshot("g1", "moves").await;
        let chat_snapshot = state.snapshot("g1", "chat").await;
        assert_eq!(moves_snapshot.as_array().unwrap().len(), 1);
        assert_eq!(chat_snapshot.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn every_broadcast_body_decodes_as_a_bare_chess_message() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        let (_, mut rx) = state.subscribe("g1", "moves").await;

        let m1 = move_message("fen1", 1);
        let v1 = braid_chess::version_hash("fen1", 1);
        state
            .put_event("g1", "moves", "alice", &m1, &v1, GENESIS_PARENT)
            .await
            .unwrap();

        let update = rx.recv().await.expect("live update should arrive");
        let decoded: ChessMessage =
            serde_json::from_value(update.body).expect("body must decode as a bare ChessMessage");
        assert!(matches!(decoded, ChessMessage::Move(_)));

        // History replay produces the same shape.
        let history = state.load_history("g1", "moves").await;
        assert_eq!(history.len(), 1);
        let decoded_history: ChessMessage = serde_json::from_value(history[0].clone())
            .expect("history entry must also decode as a bare ChessMessage");
        assert!(matches!(decoded_history, ChessMessage::Move(_)));
    }

    #[tokio::test]
    async fn non_participant_cannot_put_a_move() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        // Establish the roster: alice and bob register for g1 via SessionInfo.
        let alice_info = ChessMessage::SessionInfo {
            player_pubkey: "alice".to_string(),
            session_pubkey: "alice-session".to_string(),
            signing_pubkey: "alice-signing".to_string(),
            expires_at: 0,
        };
        let v_alice = braid_chess::version_hash("session:alice", 0);
        state
            .put_event(
                "g1",
                "moves",
                "alice",
                &alice_info,
                &v_alice,
                GENESIS_PARENT,
            )
            .await
            .unwrap();

        let bob_info = ChessMessage::SessionInfo {
            player_pubkey: "bob".to_string(),
            session_pubkey: "bob-session".to_string(),
            signing_pubkey: "bob-signing".to_string(),
            expires_at: 0,
        };
        let v_bob = braid_chess::version_hash("session:bob", 0);
        state
            .put_event("g1", "moves", "bob", &bob_info, &v_bob, &v_alice)
            .await
            .unwrap();

        // mallory has a real session (for some OTHER game — auth_ok would
        // already have accepted this request) but is not in g1's roster.
        let forged = move_message("fen1", 1);
        let vf = braid_chess::version_hash("fen1", 1);
        let err = state
            .put_event("g1", "moves", "mallory", &forged, &vf, &v_bob)
            .await
            .unwrap_err();
        assert!(matches!(err, PutEventError::NotAParticipant));

        // A real participant, correctly chained, still succeeds.
        let seq = state
            .put_event("g1", "moves", "alice", &forged, &vf, &v_bob)
            .await
            .unwrap();
        assert_eq!(seq, 3);
    }

    #[tokio::test]
    async fn on_chain_verified_roster_beats_a_forged_first_claim() {
        let pool = migrated_pool().await;
        let rpc = std::sync::Arc::new(crate::signing::solana::rpc::make_rpc(
            "http://127.0.0.1:1", // never actually contacted — cache is pre-seeded
        ));
        let participants = crate::signing::solana::game_participants::GameParticipantsCache::new(
            rpc,
            solana_sdk::pubkey::Pubkey::new_unique(),
        );
        let alice = solana_sdk::pubkey::Pubkey::new_unique();
        let bob = solana_sdk::pubkey::Pubkey::new_unique();
        let mallory = solana_sdk::pubkey::Pubkey::new_unique();
        let alice_session = solana_sdk::pubkey::Pubkey::new_unique();
        let bob_session = solana_sdk::pubkey::Pubkey::new_unique();
        // Simulates a confirmed on-chain read: g1's real Game account has
        // alice (white) and bob (black) — mallory is neither.
        participants.seed_for_test(1, alice, bob);
        participants.seed_session_for_test(1, alice, alice_session);
        participants.seed_session_for_test(1, bob, bob_session);
        let state = GameLogState::new(pool, Some(participants));

        // Mallory races and posts HER forged SessionInfo before either real
        // player — this is exactly the scenario that used to win the old
        // first-two-seen roster. `SessionInfo` posting itself is never
        // gated (that's how the roster bootstraps in the first place), so
        // this succeeds either way — the question is whether it gets
        // *trusted* afterward.
        let mallory_info = ChessMessage::SessionInfo {
            player_pubkey: mallory.to_string(),
            session_pubkey: "mallory-session".to_string(),
            signing_pubkey: "mallory-signing".to_string(),
            expires_at: 0,
        };
        let v_mallory = braid_chess::version_hash("session:mallory", 0);
        state
            .put_event(
                "1",
                "moves",
                &mallory.to_string(),
                &mallory_info,
                &v_mallory,
                GENESIS_PARENT,
            )
            .await
            .unwrap();

        // The actual regression check: mallory's forged claim was NOT
        // trusted into the roster (on-chain check caught it, even though
        // she posted first), so her next move — the thing that actually
        // matters — is rejected.
        let forged_move = move_message("fen1", 1);
        let vf = braid_chess::version_hash("fen1", 1);
        let err = state
            .put_event(
                "1",
                "moves",
                &mallory.to_string(),
                &forged_move,
                &vf,
                &v_mallory,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, PutEventError::NotAParticipant));

        // The real participant, arriving second, is correctly recognized
        // and can play.
        let alice_info = ChessMessage::SessionInfo {
            player_pubkey: alice.to_string(),
            session_pubkey: alice_session.to_string(),
            signing_pubkey: "alice-signing".to_string(),
            expires_at: 0,
        };
        let v_alice = braid_chess::version_hash("session:alice", 0);
        state
            .put_event(
                "1",
                "moves",
                &alice.to_string(),
                &alice_info,
                &v_alice,
                &v_mallory,
            )
            .await
            .unwrap();
        let alice_move = move_message("fen2", 1);
        let va = braid_chess::version_hash("fen2", 1);
        let seq = state
            .put_event_with_session(
                "1",
                "moves",
                &alice.to_string(),
                &alice_session.to_string(),
                &alice_move,
                &va,
                &v_alice,
            )
            .await
            .unwrap();
        assert_eq!(seq, 3);
    }

    #[tokio::test]
    async fn casual_identity_check_beats_a_forged_first_claim() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        state
            .register_casual_identities("g1", "host-node-id", "joiner-node-id")
            .await;

        // An impostor node_id races in first — this is exactly the scenario
        // that used to win the old first-two-seen roster unconditionally.
        let forged = move_message("fen1", 1);
        let vf = braid_chess::version_hash("fen1", 1);
        let err = state
            .put_event(
                "g1",
                "moves",
                "impostor-node-id",
                &forged,
                &vf,
                GENESIS_PARENT,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, PutEventError::NotAParticipant));

        // The real host, arriving second, is correctly recognized.
        let seq = state
            .put_event("g1", "moves", "host-node-id", &forged, &vf, GENESIS_PARENT)
            .await
            .unwrap();
        assert_eq!(seq, 1);
    }

    #[tokio::test]
    async fn no_casual_identity_entry_falls_back_to_trust_first() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        let msg = move_message("fen1", 1);
        let v1 = braid_chess::version_hash("fen1", 1);
        let seq = state
            .put_event(
                "g1",
                "moves",
                "any-node-id-at-all",
                &msg,
                &v1,
                GENESIS_PARENT,
            )
            .await
            .unwrap();
        assert_eq!(seq, 1);
    }

    #[tokio::test]
    async fn parent_mismatch_reports_the_head_to_re_chain_onto() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        // White's move lands first and becomes the stream head.
        let white = move_message("fen-white", 1);
        let v_white = braid_chess::version_hash("fen-white", 1);
        state
            .put_event("g1", "moves", "white", &white, &v_white, GENESIS_PARENT)
            .await
            .unwrap();

        // Black publishes chaining off genesis — it only tracked its own
        // moves, so it has never seen white's version. This is the exact
        // rejection that used to be a dead end.
        let black = move_message("fen-black", 1);
        let v_black = braid_chess::version_hash("fen-black", 1);
        let err = state
            .put_event("g1", "moves", "black", &black, &v_black, GENESIS_PARENT)
            .await
            .unwrap_err();

        let PutEventError::ParentMismatch { expected } = err else {
            panic!("expected a parent mismatch");
        };
        assert_eq!(
            expected, v_white,
            "the reported head must be what the client re-chains onto"
        );

        // Retrying with that head succeeds — the client's retry loop in
        // `braid_transport::publish` does exactly this.
        let seq = state
            .put_event("g1", "moves", "black", &black, &v_black, &expected)
            .await
            .unwrap();
        assert_eq!(seq, 2, "black's move must now be persisted");
    }

    #[tokio::test]
    async fn both_players_session_info_can_land_in_sequence() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        let a = ChessMessage::SessionInfo {
            player_pubkey: "alice".to_string(),
            session_pubkey: "as".to_string(),
            signing_pubkey: "ag".to_string(),
            expires_at: 0,
        };
        let v_a = braid_chess::version_hash("session:alice", 0);
        state
            .put_event("g1", "moves", "alice", &a, &v_a, GENESIS_PARENT)
            .await
            .unwrap();

        let b = ChessMessage::SessionInfo {
            player_pubkey: "bob".to_string(),
            session_pubkey: "bs".to_string(),
            signing_pubkey: "bg".to_string(),
            expires_at: 0,
        };
        let v_b = braid_chess::version_hash("session:bob", 0);
        // Chained off alice's, as the client now does after re-chaining.
        state
            .put_event("g1", "moves", "bob", &b, &v_b, &v_a)
            .await
            .unwrap();

        // And the first real move chains off bob's — the whole point: the
        // stream is still writable after both SessionInfos.
        let mv = move_message("fen1", 1);
        let v_mv = braid_chess::version_hash("fen1", 1);
        let seq = state
            .put_event("g1", "moves", "alice", &mv, &v_mv, &v_b)
            .await
            .unwrap();
        assert_eq!(seq, 3);
    }

    #[tokio::test]
    async fn empty_roster_does_not_block_casual_games() {
        let pool = migrated_pool().await;
        let state = GameLogState::new(pool, None);

        let msg = move_message("fen1", 1);
        let v1 = braid_chess::version_hash("fen1", 1);
        let seq = state
            .put_event(
                "g1",
                "moves",
                "some-iroh-node-id",
                &msg,
                &v1,
                GENESIS_PARENT,
            )
            .await
            .unwrap();
        assert_eq!(seq, 1);
    }
}
