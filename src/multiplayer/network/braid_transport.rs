//! Braid-backed move/resign/chat transport.
//!
//! Replaces the old VPS relay mailbox (`relay_bridge.rs`, removed) with the
//! durable, push-based `GET`/`PUT /game/:id/moves` and `/game/:id/chat`
//! backend added in this migration
//! (`backend/src/signing/routes/game_log.rs`). Gossip stays the fast,
//! optimistic path; this is the reliable fallback/catch-up path — same role
//! the relay played, but pushed instead of polled, and durable across a
//! backend restart instead of dropped.
//!
//! # Why this doesn't use `braid_chess::ChessPublisher`
//!
//! `ChessPublisher` (the shared crate's existing PUT-side client) sends the
//! bare `ChessMessage` as the body with `Version`/`Parents` conveyed via
//! HTTP request headers, and has no authentication mechanism at all. Our
//! backend's auth (`GameLogState`/`auth_ok` in `game_log.rs`) is real and
//! load-bearing for the wagered-game path — extending `ChessPublisher` to
//! carry auth headers it was never designed for is more shared-crate
//! surgery than a dedicated, already-tested small client here. [`publish`]
//! below sends the same `GameEventReq` JSON body shape the backend expects
//! and already has regression tests for.
//!
//! # Why Braid moves don't go through [`super::reorder::NonceSequencer`]
//!
//! `NonceSequencer` reconciles arrivals by a P2P `nonce` field. Braid's
//! `ChessMessage::Move` (`MovePayload`) has no such field — it's a
//! different wire type, shared with chat/clock/engine. But it doesn't need
//! one: the backend's `AppendLog`-free broadcast channel
//! (`GameLogState::put_event`) already delivers to a live subscriber in
//! true append order, exactly once. So a Braid-delivered move is applied
//! directly, without buffering — the one thing that *does* need handling is
//! a move delivered by both gossip and Braid, which
//! `CausalChainState::applied_versions` catches (see its doc comment).
//!
//! # Reconnect
//!
//! Unlike `ChessSubscriber`'s bare usage elsewhere (chat/tournament
//! streams, which have no reconnect logic — a dropped connection just stops
//! delivering forever), [`spawn_reconnecting_subscription`] retries with
//! bounded exponential backoff. Because the backend replays full history to
//! every new subscriber, a reconnect is automatically a correct catch-up.

use bevy::prelude::*;
use braid_chess::message::ChatPayload;
use braid_chess::{ChessMessage, ChessSubscriber, MovePayload};
use serde::Serialize;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::multiplayer::network::online_game_session::{OnlineChatMessage, OnlineGameSession};
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::types::{CausalChainState, NetworkEvent};
use crate::multiplayer::TokioRuntime;

const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

// ── Publish (PUT) ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GameEventReq<'a> {
    player_pubkey: &'a str,
    session_token: &'a str,
    message: &'a ChessMessage,
    content_version: &'a str,
    content_parent: &'a str,
}

/// Fire-and-forget PUT, mirroring `relay_send`'s reliability posture: errors
/// are logged, not fatal — gossip may still deliver the same event.
fn publish(
    base_url: String,
    game_id: String,
    stream: &'static str,
    player_pubkey: String,
    session_token: String,
    message: ChessMessage,
    content_version: String,
    content_parent: String,
) {
    std::thread::spawn(move || {
        let client = match crate::multiplayer::network::vps::client() {
            Ok(c) => c,
            Err(e) => {
                warn!("[braid-transport] client build failed: {e}");
                return;
            }
        };
        let url = format!(
            "{}/game/{}/{}",
            base_url.trim_end_matches('/'),
            game_id,
            stream
        );
        let body = GameEventReq {
            player_pubkey: &player_pubkey,
            session_token: &session_token,
            message: &message,
            content_version: &content_version,
            content_parent: &content_parent,
        };
        match client.put(&url).json(&body).send() {
            Ok(resp) if resp.status().is_success() => {
                debug!("[braid-transport] published to {game_id}/{stream}");
            }
            Ok(resp) => warn!(
                "[NET] Move sync backup (Braid) couldn't save {stream} for game {game_id}: server returned HTTP {}",
                resp.status()
            ),
            Err(e) => warn!(
                "[NET] Move sync backup (Braid) couldn't save {stream} for game {game_id}: {e}"
            ),
        }
    });
}

pub fn publish_move(
    base_url: String,
    game_id: String,
    player_pubkey: String,
    session_token: String,
    payload: MovePayload,
    content_version: String,
    content_parent: String,
) {
    publish(
        base_url,
        game_id,
        "moves",
        player_pubkey,
        session_token,
        ChessMessage::Move(payload),
        content_version,
        content_parent,
    );
}

pub fn publish_resign(
    base_url: String,
    game_id: String,
    player_pubkey: String,
    session_token: String,
    resigning_player: String,
    content_parent: String,
) {
    let content_version = braid_chess::version_hash(&format!("resign:{resigning_player}"), 0);
    publish(
        base_url,
        game_id,
        "moves",
        player_pubkey,
        session_token,
        ChessMessage::Resign {
            player: resigning_player,
        },
        content_version,
        content_parent,
    );
}

pub fn publish_chat(
    base_url: String,
    game_id: String,
    player_pubkey: String,
    session_token: String,
    player: String,
    text: String,
    timestamp_ms: u64,
    content_parent: String,
) {
    let content_version = braid_chess::version_hash(&format!("chat:{player}:{timestamp_ms}"), 0);
    publish(
        base_url,
        game_id,
        "chat",
        player_pubkey,
        session_token,
        ChessMessage::Chat(ChatPayload {
            player,
            text,
            timestamp_ms,
        }),
        content_version,
        content_parent,
    );
}

/// SessionInfo is sent once, near game start, before any moves exist — its
/// causal parent is always genesis (see the call site's comment in
/// `solana/integration/systems.rs`).
#[allow(clippy::too_many_arguments)]
pub fn publish_session_info(
    base_url: String,
    game_id: String,
    player_pubkey: String,
    session_token: String,
    wallet_pubkey: String,
    session_pubkey: String,
    signing_pubkey: String,
    expires_at: i64,
) {
    let content_version = braid_chess::version_hash(&format!("session:{wallet_pubkey}"), 0);
    publish(
        base_url,
        game_id,
        "moves",
        player_pubkey,
        session_token,
        ChessMessage::SessionInfo {
            player_pubkey: wallet_pubkey,
            session_pubkey,
            signing_pubkey,
            expires_at,
        },
        content_version,
        "0".to_string(),
    );
}

// ── Subscribe (with reconnect) ─────────────────────────────────────────────

/// Bridges the Braid moves+chat subscriptions for the active game into the
/// same Bevy message bus gossip feeds, with reconnect-on-drop.
#[derive(Resource, Default)]
pub struct BraidTransportState {
    game_id: String,
    rx: Option<crossbeam_channel::Receiver<ChessMessage>>,
    /// True while the moves+chat subscriptions are both live. Shared with
    /// the spawned reconnect task via `Arc` since it runs on Tokio, not the
    /// Bevy main thread.
    ///
    /// Read by `tick_heartbeat` (`systems.rs`) so a "relay-only" game (Iroh
    /// gossip down, Braid up) doesn't get falsely declared disconnected —
    /// this is the exact same real, previously-reproduced failure mode the
    /// old relay's Ping/Pong fallback existed to prevent (see that removal
    /// site's comment), just answered by a real connectivity signal instead
    /// of re-adding a heartbeat message type to this transport.
    pub connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl BraidTransportState {
    /// Test-only constructor: wires a caller-controlled `rx` directly,
    /// bypassing the real HTTP subscription machinery, so tests can drive
    /// `drain_braid_messages` with a synthetic message stream.
    #[cfg(test)]
    pub(crate) fn new_for_test(
        game_id: String,
        rx: crossbeam_channel::Receiver<ChessMessage>,
    ) -> Self {
        Self {
            game_id,
            rx: Some(rx),
            connected: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    pub fn reset(&mut self) {
        self.game_id.clear();
        self.rx = None;
        self.connected
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Start (or restart, if `game_id` changed) the reconnecting Braid
/// subscriptions for the active game. Idempotent for the same `game_id`.
///
/// `rt` must be the shared [`TokioRuntime`] handle — see
/// `social.rs::LobbyChatSession::activate`'s doc comment for why: the
/// underlying `reqwest`-based `BraidClient` needs a live Tokio reactor,
/// which Bevy's own task pools don't provide.
pub fn ensure_subscribed(
    state: &mut BraidTransportState,
    base_url: String,
    game_id: String,
    rt: &tokio::runtime::Handle,
) {
    if state.game_id == game_id && state.rx.is_some() {
        return;
    }
    state.game_id = game_id.clone();
    state
        .connected
        .store(false, std::sync::atomic::Ordering::Relaxed);

    let (tx, rx) = crossbeam_channel::unbounded::<ChessMessage>();
    state.rx = Some(rx);

    spawn_reconnecting_subscription(base_url, game_id, tx, state.connected.clone(), rt);
}

fn spawn_reconnecting_subscription(
    base_url: String,
    game_id: String,
    tx: crossbeam_channel::Sender<ChessMessage>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
    rt: &tokio::runtime::Handle,
) {
    rt.spawn(async move {
        let mut backoff = MIN_BACKOFF;
        // Only true once we've actually been connected and lost it — gates
        // the "reconnected" notice so the very first connect of a match
        // (the common case, nothing wrong) stays quiet.
        let mut recovering = false;
        loop {
            let sub = match ChessSubscriber::new(&base_url, &game_id) {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        "[NET] Move sync backup (Braid) couldn't start for game {game_id}: {e}"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                    recovering = true;
                    continue;
                }
            };

            // Run both streams concurrently on this task; if either drops,
            // reconnect both (simplest correct behavior — the backend
            // replays full history on resubscribe, so this is never a
            // silent gap, just a brief reconnect delay).
            let moves = sub.subscribe_moves().await;
            let chat = sub.subscribe_chat().await;
            let (moves_rx, chat_rx) = match (moves, chat) {
                (Ok((m, _)), Ok((c, _))) => (m, c),
                (Err(e), _) | (_, Err(e)) => {
                    warn!(
                        "[NET] Move sync backup (Braid) couldn't reach the server for game {game_id}: {e} — retrying"
                    );
                    connected.store(false, std::sync::atomic::Ordering::Relaxed);
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                    recovering = true;
                    continue;
                }
            };
            if recovering {
                info!("[NET] Move sync backup (Braid) reconnected for game {game_id}");
            } else {
                debug!("[braid-transport] subscribed to moves+chat for game {game_id}");
            }
            backoff = MIN_BACKOFF; // reset after a successful (re)connect
            connected.store(true, std::sync::atomic::Ordering::Relaxed);

            loop {
                tokio::select! {
                    msg = moves_rx.recv() => {
                        match msg {
                            Ok(m) => { let _ = tx.send(m); }
                            Err(_) => break,
                        }
                    }
                    msg = chat_rx.recv() => {
                        match msg {
                            Ok(m) => { let _ = tx.send(m); }
                            Err(_) => break,
                        }
                    }
                }
            }
            connected.store(false, std::sync::atomic::Ordering::Relaxed);
            warn!(
                "[NET] Move sync backup (Braid) lost connection for game {game_id} — reconnecting"
            );
            recovering = true;
            tokio::time::sleep(MIN_BACKOFF).await;
        }
    });
}

/// Drain Braid-delivered moves/resigns/chat into the same event types
/// gossip produces, deduping cross-transport against
/// `CausalChainState::applied_versions`.
pub fn drain_braid_messages(
    state: Res<BraidTransportState>,
    session: Res<OnlineGameSession>,
    mut causal: ResMut<CausalChainState>,
    mut network_events: MessageWriter<NetworkEvent>,
    mut resign_events: MessageWriter<crate::game::events::ResignEvent>,
    mut chat_events: MessageWriter<OnlineChatMessage>,
) {
    let Some(rx) = &state.rx else {
        return;
    };
    let game_id_u64 =
        crate::multiplayer::network::online_game_session::numeric_game_id(&session.game_id);

    while let Ok(msg) = rx.try_recv() {
        match msg {
            ChessMessage::Move(payload) => {
                let version = braid_chess::version_hash(&payload.fen_after, payload.move_number);
                let first_time_seen = causal
                    .applied_versions
                    .entry(game_id_u64)
                    .or_default()
                    .insert(version);
                if !first_time_seen {
                    continue; // already applied via gossip — see module doc comment
                }
                network_events.write(NetworkEvent::MessageReceived(NetworkMessage::Move {
                    game_id: game_id_u64,
                    turn: payload.move_number as u16,
                    move_uci: payload.uci,
                    next_fen: payload.fen_after,
                    nonce: 0,
                    timestamp_ms: 0,
                    agent_id: Vec::new(),
                    seq: 0,
                    parent_version: String::new(),
                }));
            }
            ChessMessage::Resign { player } => {
                let winner = if player == "white" { "black" } else { "white" };
                resign_events.write(crate::game::events::ResignEvent {
                    winner: winner.to_string(),
                    remote: true,
                });
            }
            ChessMessage::Chat(payload) => {
                chat_events.write(OnlineChatMessage {
                    player: payload.player,
                    text: payload.text,
                    timestamp_ms: payload.timestamp_ms,
                });
            }
            ChessMessage::SessionInfo { signing_pubkey, .. } => {
                // Mirrors `handle_network_events`'s gossip-side roster
                // building exactly (`systems.rs`) — this is the durable
                // fallback for the same real bug described at this
                // message's publish site (gossip alone can silently drop
                // SessionInfo before the P2P link establishes).
                let Ok(key) = bs58::decode(&signing_pubkey).into_vec() else {
                    warn!("[braid-transport] SessionInfo had an unparseable signing_pubkey");
                    continue;
                };
                let entry = causal.roster.entry(game_id_u64).or_default();
                if !entry.contains(&key) && entry.len() < 2 {
                    entry.push(key);
                    info!(
                        "[braid-transport] Roster for game {} now has {} entry(ies) via Braid",
                        game_id_u64,
                        entry.len()
                    );
                }
            }
            // OfferDraw/AcceptDraw/DeclineDraw/Clock/EngineAnalysis aren't
            // published through this transport (draw offers stay
            // gossip-only for now; Clock/EngineAnalysis are separate Braid
            // streams this module doesn't subscribe to).
            _ => {}
        }
    }
}

/// Starts (or restarts, on `game_id` change) the reconnecting Braid
/// subscription whenever `OnlineGameSession` becomes configured for a new
/// game. Reactive rather than called from `start_session` directly — that
/// function is a plain helper with four call sites across lobby/tournament
/// flows; watching the resource here avoids threading `TokioRuntime`/
/// `BraidTransportState` through all of them.
fn sync_braid_subscription(
    mut state: ResMut<BraidTransportState>,
    session: Res<OnlineGameSession>,
    tokio_runtime: Res<TokioRuntime>,
) {
    if !session.is_configured() {
        return;
    }
    ensure_subscribed(
        &mut state,
        session.base_url.clone(),
        session.game_id.clone(),
        tokio_runtime.0.handle(),
    );
}

/// Registers the Braid transport resource and its systems.
pub struct BraidTransportPlugin;

impl Plugin for BraidTransportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BraidTransportState>()
            .add_systems(Update, (sync_braid_subscription, drain_braid_messages));
    }
}
