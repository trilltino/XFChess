//! Durable, ordered game event log over Braid-HTTP 209 — replaces both the
//! old poll-based `p2p_relay` mailbox and `chat.rs`'s ephemeral,
//! restart-losing `ChatRelayState`.
//!
//! Routes:
//!   GET  /game/:id/moves — Braid-209 subscription; new subscribers get the
//!                          full persisted move/resign/draw history replayed
//!                          as individual chunks, then live updates.
//!   PUT  /game/:id/moves — publish a move-stream event (any `ChessMessage`
//!                          variant except `Chat`/`Clock`/`EngineAnalysis`).
//!   GET  /game/:id/chat  — same shape, now backed by real persistence
//!                          instead of an in-memory-only broadcast channel.
//!   PUT  /game/:id/chat  — publish a chat message.
//!
//! ## Why this doesn't use `xfchess_braid_server::ResourceHub`
//!
//! `ResourceHub`/`AppendLog` is a generic ordered-list resource store, but
//! it wraps every update as an RFC 6902 JSON-Patch document (`[{"op":"add",
//! "path":"/-","value":entry}]`) and its subscribe-time snapshot is a single
//! chunk whose body is the *array* of every existing entry. The existing
//! client, `braid_chess::ChessSubscriber` (shared with the chat/clock/engine
//! streams), only ever parses a chunk body as one bare `ChessMessage` — it
//! has no JSON-Patch unwrapping and no bulk-array handling. Nothing
//! previously subscribed to an `AppendLog` through `ChessSubscriber`, so
//! this mismatch was never exercised until this module. Rather than modify
//! `ChessSubscriber` (shared with three other streams) or fight the
//! abstraction, this module keeps its own `broadcast::Sender<BraidUpdate>`
//! map — the same wire shape `chat.rs` always used and that the client
//! already decodes correctly — and adds SQLite persistence, which the old
//! chat relay never had.
//!
//! ## Two different "version" concepts — don't conflate them
//!
//! [`xfchess_braid_server::resource::protocol::Version`] is a `u64` used
//! for the wire-protocol `Version`/`Parents` chunk headers here (the
//! `game_event_log.seq` column) — it answers "did this arrive after that."
//!
//! `content_version`/`content_parent` (on [`GameEventReq`] and
//! [`GameLogState::put_event`]) are [`braid_chess::version_hash`] SHA-256
//! hex strings — the *causal* identity of a move (same content as
//! `CausalChainState::head_version` on the P2P side, and the same shape of
//! check as `parent_nonce` in the Solana program's `RecordMove`
//! instruction). They answer "is this move a valid continuation of the
//! position it claims to follow," and are what [`GameLogState::put_event`]
//! validates against the log's current head before accepting a write — the
//! third independent layer (P2P gossip, this backend log, on-chain)
//! enforcing the same causal-chain invariant.
//!
//! ## Persistence
//!
//! Every accepted event is written to the `game_event_log` SQLite table
//! (`backend/migrations/027_game_event_log.sql`) *before* being broadcast —
//! so a subscriber can never observe an event that a crash immediately
//! afterward would then lose. History is read fresh from SQLite on every
//! subscribe (no in-memory cache to go stale) — cheap enough since this
//! isn't a hot path.
//!
//! ## Design note
//!
//! The core persistence/broadcast/causal-check logic lives on
//! [`GameLogState`] itself and only depends on a `SqlitePool` — not on the
//! full `AppState` — so it's directly unit-testable (see the `tests`
//! module). The axum handlers below are thin: extract state, check auth,
//! delegate.

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

/// Genesis parent value for a game's first event — matches the P2P causal
/// chain's convention (`CausalChainState`/`systems.rs`) so the same string
/// means the same thing ("no prior event") on both the P2P and backend log.
const GENESIS_PARENT: &str = "0";

/// Why a `PUT` was rejected before it reached persistence.
#[derive(Debug)]
pub enum PutEventError {
    /// `content_parent` didn't match the log's current head for this game —
    /// the sender's view of the chain is stale. Mirrors the on-chain
    /// `parent_nonce` mismatch and the P2P equivocation check.
    ParentMismatch {
        expected: String,
    },
    /// The poster isn't one of this game's two registered participants —
    /// they hold a valid platform session, just not for this `game_id`.
    /// Formally: `specs/BraidChain.tla`'s Finding 4
    /// (`OnlyParticipantsAccepted`, `AuthCheck` switch).
    NotAParticipant,
    Db(sqlx::Error),
}

/// Move-stream kinds that require the poster to be a registered
/// participant of the game. `session_info` is how the roster gets
/// populated in the first place (checking it against itself would
/// deadlock); `chat`/`clock`/`engine_analysis` aren't gameplay actions.
fn requires_participant_check(kind: &str) -> bool {
    matches!(
        kind,
        "move" | "resign" | "offer_draw" | "accept_draw" | "decline_draw"
    )
}

/// Backend-side state for the durable game event log: the SQLite pool it
/// persists through, one live broadcast channel per `game_id`/stream pair
/// (created lazily, on first subscribe or publish), and a per-game
/// participant roster.
pub struct GameLogState {
    pool: sqlx::SqlitePool,
    channels: RwLock<HashMap<String, broadcast::Sender<BraidUpdate>>>,
    /// game_id → up to two wallet pubkey strings verified as authorized for
    /// this game — a cache, not a trust source. For a wagered game, an
    /// entry only lands here after independently matching the on-chain
    /// `Game` account's `white`/`black` (`participants`, below) — see
    /// `check_participant`. For a casual game (no on-chain `Game` account),
    /// this remains the original first-two-`SessionInfo`-seen bootstrap —
    /// a known, lower-severity, documented gap (no money at stake), tracked
    /// separately (`docs/plans/networking-hardening-plan.md`'s Phase D).
    roster: RwLock<HashMap<String, Vec<String>>>,
    session_roster: RwLock<HashMap<String, HashMap<String, String>>>,
    /// On-chain-verified `(white, black)` lookups, cached. `None` in tests
    /// / anywhere a live RPC client isn't available — falls back to the
    /// original SessionInfo-only bootstrap behavior for every game in that
    /// case, matching this module's pre-Phase-B behavior exactly.
    participants: Option<crate::signing::solana::game_participants::GameParticipantsCache>,
    /// game_id → `(host_node_id, joiner_node_id)`, populated by
    /// `p2p_relay::routes::accept_join` once a game's JOIN_ACK handshake —
    /// cryptographically signed since Phase A
    /// (`docs/plans/networking-hardening-plan.md`) — completes.
    ///
    /// Phase D: the casual-game (no wallet, no on-chain `Game` account)
    /// counterpart to `participants` above. Without this, a casual game's
    /// roster had no ground truth at all to check a claim against — purely
    /// first-two-`SessionInfo`/move-senders-seen, so any node_id that got a
    /// message in before the real players occupied a trusted slot. A
    /// direct-connection game (never announced to the lobby, so never goes
    /// through `accept_join`) has no entry here — falls back to the
    /// original first-two-seen trust, same as before Phase D existed.
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

    /// Records a JOIN_ACK-verified node-id pair for `game_id` — called once
    /// by `p2p_relay::routes::accept_join` per game, before that game's
    /// `ActiveGame` lobby record becomes eligible for TTL eviction (Braid,
    /// not `p2p_relay`, carries moves/chat now, so a wagered-or-casual match
    /// in progress generates no further lobby traffic to keep it alive).
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

    /// Persisted history for a stream, oldest first, decoded as raw
    /// `ChessMessage` JSON values (each individually shaped exactly like a
    /// live update body — no wrapping).
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

    /// Subscribe: returns persisted history (oldest first) plus a live
    /// receiver for everything published from this point on. There is a
    /// narrow window in which an event published between reading history
    /// and registering the receiver could appear in neither — acceptable
    /// for the same reason a fresh full-FEN resync already covers gaps at
    /// the P2P layer; not acceptable to leave silently undocumented, so:
    /// this is the one known gap in this module's exactly-once guarantee.
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

    /// Current JSON snapshot without subscribing (plain GET fallback).
    pub async fn snapshot(&self, game_id: &str, stream: &str) -> serde_json::Value {
        serde_json::Value::Array(self.load_history(game_id, stream).await)
    }

    /// Validate the poster is a registered participant (for gameplay
    /// kinds) and causal continuity against the log's current head,
    /// persist, then broadcast to live subscribers. Returns the assigned
    /// `seq` on success. Assumes the caller has already authenticated the
    /// sender (`auth_ok`) — this additionally verifies they're authorized
    /// for *this* `game_id` specifically, which `auth_ok` alone does not
    /// (Finding 4, `specs/BraidChain.tla`).
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

    /// Verify `player_pubkey` is authorized to act in `game_id`.
    ///
    /// Phase B fix (`docs/plans/networking-hardening-plan.md`): for a real
    /// on-chain (wagered) game, checks — and caches — against the `Game`
    /// account's `white`/`black` directly instead of trusting whoever
    /// claimed a roster slot first. This is what actually closes the race:
    /// a forged `SessionInfo` simply won't match on-chain truth, so it's
    /// rejected outright rather than winning a timing race. For a casual
    /// game (no on-chain `Game` account — `game_id` doesn't resolve, or no
    /// `participants` cache is configured at all), falls back to the
    /// original first-two-`SessionInfo`-seen roster, unchanged.
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

    /// Called for every accepted `SessionInfo` post — this is how the
    /// roster gets populated in the first place. Crucially does **not**
    /// blindly trust the claim the way the pre-Phase-B code did: a forged
    /// claim for a real on-chain (wagered) game is checked and discarded,
    /// not added. The `SessionInfo` PUT itself always still succeeds either
    /// way (posting isn't gated the way gameplay actions are) — this only
    /// controls whether the claim gets *trusted* afterward.
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

    /// Checks `player_pubkey` against whichever source of ground truth is
    /// available for `game_id`: the on-chain `Game` account first (Phase B,
    /// wagered games), then the JOIN_ACK-verified casual-identity pair
    /// (Phase D, casual games) if the former found nothing to check against.
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

    /// Phase D counterpart to `on_chain_check`: checks `player_pubkey`
    /// (an Iroh node_id string for a casual game — see `check_participant`)
    /// against the JOIN_ACK-verified pair `register_casual_identities`
    /// recorded for `game_id`, if any.
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

    /// Checks `player_pubkey` against `game_id`'s on-chain `Game` account,
    /// if one exists and a participants cache is configured.
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
    /// No on-chain data available: no participants cache configured, or
    /// `game_id` doesn't resolve to a real `Game` account (casual game).
    Unavailable,
    /// This wallet is `white` or `black` for this game, on-chain.
    Verified,
    /// A real on-chain game exists and this wallet is neither.
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
    /// Who is claiming to publish this event. **Not always a Solana pubkey**
    /// despite the wire name: for a wagered game it is the wallet pubkey
    /// (checked against on-chain `Game.white`/`black`), but for a casual game
    /// it is an Iroh node-id string (checked against the JOIN_ACK-verified
    /// pair). `check_participant` handles both; `auth_ok` only treats it as a
    /// wallet when it parses as one.
    #[serde(rename = "player_pubkey")]
    pub sender_identity: String,
    /// Session pubkey returned by global-session/activate — proves the
    /// caller owns this wallet (same check chat.rs used to do).
    ///
    /// Currently always empty: no client populates it (see `auth_ok`).
    pub session_token: String,
    pub message: ChessMessage,
    /// `braid_chess::version_hash(..)` computed client-side for this event.
    pub content_version: String,
    /// The sender's last known head version for this game (`"0"` for the
    /// first event ever sent).
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

/// Body of a `409 CONFLICT` from either PUT route: the stream head the
/// rejected event should have named as its `content_parent`. Consumed by
/// the client's `braid_transport::publish` retry.
#[derive(Debug, Serialize, Deserialize)]
pub struct ParentMismatchResp {
    pub expected_parent: String,
}

/// Phase C (`docs/plans/networking-hardening-plan.md`): lets the client seed
/// its own P2P/gossip roster (`CausalChainState::verified_wallets`) from the
/// same on-chain truth `check_participant` already verifies claims against
/// server-side (Phase B) — closing the roster-building race on the gossip
/// side too, not just this backend's Braid log. 404 for a casual game (no
/// on-chain `Game` account) or a malformed `game_id`; the client falls back
/// to its original trust-first bootstrap in that case.
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

/// SQLITE_BUSY / SQLITE_BUSY_SNAPSHOT (code 517) under concurrent access.
/// The pool's `busy_timeout` PRAGMA (`infrastructure/database.rs`) already
/// waits out ordinary lock contention, but that does not cover a WAL read
/// snapshot going stale mid-transaction — busy_timeout makes SQLite wait
/// longer for the *same* transaction to acquire a lock, but a stale
/// snapshot never becomes valid no matter how long it waits. The only fix
/// is a fresh `BEGIN` (a new snapshot), which is why this is retried at the
/// handler level, not inside `put_event`'s own transaction.
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

/// Casual (non-wagered) online games have no Solana wallet at all — the old
/// `p2p_relay` this replaced only ever matched on a claimed Iroh node-id
/// string, no wallet required. Requiring a wallet session unconditionally
/// here would 401 every casual-game move. So: `player_pubkey` that parses as
/// a real Solana pubkey must have a matching active session (this is the
/// wagered-game path, and it's real auth — strictly more than the old relay
/// had, which trusted the claimed identity outright); anything else is
/// treated as a casual/non-wallet identity and passes through, matching the
/// old relay's trust model exactly (not a new hole, not a regression).
/// Same fail-open-when-not-applicable shape as `require_relay_or_jwt`
/// (`infrastructure/auth_middleware.rs`) — see that function's doc comment.
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

    /// Simulates a backend restart: persist two moves via one
    /// `GameLogState`, drop it, then construct a fresh one sharing only the
    /// SQLite pool and confirm the full history comes back — the property
    /// this migration exists to add (the old relay/chat state had none).
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

    /// The actual wire-format guarantee this module exists to provide:
    /// every chunk body (history replay AND live) must decode as a bare
    /// `ChessMessage` — exactly what `ChessSubscriber::decode_update`
    /// parses. Regression test for the ResourceHub/AppendLog incompatibility
    /// this module was rewritten to avoid.
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

    /// Finding 4 (`specs/BraidChain.tla`, `OnlyParticipantsAccepted`): a
    /// player who is registered for a DIFFERENT game must not be able to
    /// PUT a move into this one, even with an otherwise-valid causal chain.
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

    /// The actual vulnerability Phase B closes: a forged `SessionInfo` that
    /// WINS the arrival-order race (lands before either real player's) must
    /// still be rejected once there's on-chain-verified truth to check it
    /// against — unlike `non_participant_cannot_put_a_move` above, where
    /// mallory is caught only because alice/bob's SessionInfo already beat
    /// her to the (pre-Phase-B) roster. Here mallory goes FIRST, and it
    /// still doesn't matter.
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

    /// Phase D (`docs/plans/networking-hardening-plan.md`): once
    /// `register_casual_identities` has recorded the real (JOIN_ACK-
    /// verified) node-id pair for a casual game — mirroring what
    /// `accept_join` does — a third node_id claiming to be a participant is
    /// rejected outright, even if it PUTs before either real player does.
    /// This is the casual-game counterpart to
    /// `on_chain_verified_roster_beats_a_forged_first_claim`.
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

    /// A game_id that never went through `accept_join` (direct-connection
    /// mode, which never announces to the lobby at all — see
    /// `render_p2p_joiner_waiting_screen`'s doc comment, client-side) has no
    /// casual-identity entry — must fall back to the original
    /// first-two-seen trust, unchanged, not start rejecting everyone.
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

    /// The shape the client's re-chain retry depends on: a rejected event
    /// must report the head it *should* have named. Both players write to one
    /// shared moves stream, so a publisher genuinely cannot compute this
    /// itself — without it, the client can only give up, which is what made
    /// the durable transport silently stop replicating after the first
    /// player's first event.
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

    /// Both players post a `SessionInfo` at game start. The second one used
    /// to claim genesis as its parent and be rejected, which wedged the whole
    /// moves stream of every wagered game before move one.
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

    /// Casual (no-wallet) games never post SessionInfo, so the roster for
    /// that game_id stays empty — the participant check must not block
    /// them (mirrors the P2P roster's identical bootstrap behavior).
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
