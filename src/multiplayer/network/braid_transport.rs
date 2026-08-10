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

/// Genesis parent for a stream's first event — must match `GENESIS_PARENT`
/// in the backend's `game_log.rs`.
const GENESIS_PARENT: &str = "0";

/// How many times a publish re-chains onto a server-reported head before
/// giving up. Each retry costs a round trip, and a move that loses this many
/// races in a row is better left to gossip (or to the opponent's next
/// subscribe-time history replay) than retried indefinitely.
const MAX_PARENT_RETRIES: usize = 3;

// ── Stream head tracking ──────────────────────────────────────────────────

/// The causal head of each Braid stream for the active game, as far as this
/// client knows.
///
/// # Why this can't live on `OnlineGameSession`
///
/// The backend validates `content_parent` against the head of the *whole
/// shared stream* — both players write to `/game/:id/moves`. A head tracked
/// from only our own publishes is therefore wrong the moment the opponent
/// writes anything, and the backend answers every later publish with `409
/// CONFLICT`. That was the original bug: `OnlineGameSession::last_version`
/// advanced on local moves only, so in a casual game the second player's very
/// first move was rejected and never persisted, and in a wagered game the two
/// `SessionInfo` posts (both claiming genesis as parent) wedged the stream
/// before move one — every later publish then failed forever. Because
/// [`publish`] is fire-and-forget, nothing surfaced: moves simply never
/// replicated through the durable path.
///
/// So the head is tracked here, advanced from *delivered* messages (i.e. the
/// server's accepted order, opponent's writes included), and shared with the
/// publish threads.
pub struct BraidStreamHeads {
    moves: String,
    chat: String,
    /// Versions this client published. The backend echoes every accepted
    /// event back to *all* subscribers including the publisher, so without
    /// this we re-ingest our own moves as though they came from the opponent
    /// (see [`drain_braid_messages`]).
    self_published: std::collections::HashSet<String>,
}

impl Default for BraidStreamHeads {
    fn default() -> Self {
        Self {
            moves: GENESIS_PARENT.to_string(),
            chat: GENESIS_PARENT.to_string(),
            self_published: std::collections::HashSet::new(),
        }
    }
}

impl BraidStreamHeads {
    fn head(&self, stream: &str) -> String {
        if stream == "chat" {
            self.chat.clone()
        } else {
            self.moves.clone()
        }
    }

    fn set_head(&mut self, stream: &str, version: String) {
        if stream == "chat" {
            self.chat = version;
        } else {
            self.moves = version;
        }
    }
}

/// Shared handle: publish runs on its own thread, draining runs on the Bevy
/// main thread.
pub type SharedStreamHeads = std::sync::Arc<std::sync::Mutex<BraidStreamHeads>>;

/// Recompute the `content_version` a message was published under. Must mirror
/// the `version_hash` inputs used by the `publish_*` helpers below exactly —
/// this is how a *delivered* message is matched back to the head and
/// self-published bookkeeping, since `ChessMessage` carries no version field
/// on the wire.
fn version_of(message: &ChessMessage) -> Option<String> {
    Some(match message {
        ChessMessage::Move(p) => braid_chess::version_hash(&p.fen_after, p.move_number),
        ChessMessage::Resign { player } => {
            braid_chess::version_hash(&format!("resign:{player}"), 0)
        }
        ChessMessage::Chat(p) => {
            braid_chess::version_hash(&format!("chat:{}:{}", p.player, p.timestamp_ms), 0)
        }
        ChessMessage::SessionInfo { player_pubkey, .. } => {
            braid_chess::version_hash(&format!("session:{player_pubkey}"), 0)
        }
        _ => return None,
    })
}

/// Which Braid stream a message belongs to. Mirrors the backend's
/// `belongs_to_stream`.
fn stream_of(message: &ChessMessage) -> &'static str {
    if matches!(message, ChessMessage::Chat(_)) {
        "chat"
    } else {
        "moves"
    }
}

// ── Publish (PUT) ─────────────────────────────────────────────────────────

/// Mirrors the backend's `game_log::GameEventReq`. Field names must stay in
/// sync with that type's serde names, not its Rust names.
#[derive(Serialize)]
struct GameEventReq<'a> {
    /// Wallet pubkey for a wagered game, Iroh node id for a casual one — see
    /// the backend type's doc comment.
    #[serde(rename = "player_pubkey")]
    sender_identity: &'a str,
    session_token: &'a str,
    message: &'a ChessMessage,
    content_version: &'a str,
    content_parent: &'a str,
}

/// Body of the backend's `409 CONFLICT`.
#[derive(serde::Deserialize)]
struct ParentMismatchResp {
    expected_parent: String,
}

/// Fire-and-forget PUT: errors are logged, not fatal — gossip may still
/// deliver the same event.
///
/// The parent is read from [`BraidStreamHeads`] at send time rather than
/// passed in, because the correct parent is the *shared* stream head and only
/// this transport tracks it. On `409 CONFLICT` the backend replies with the
/// head it actually expected; we adopt it and retry, which is what makes two
/// players writing to one stream work at all.
fn publish(
    base_url: String,
    game_id: String,
    stream: &'static str,
    sender_identity: String,
    session_token: String,
    message: ChessMessage,
    content_version: String,
    heads: SharedStreamHeads,
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

        // Claim the version up-front so the echo of our own event is
        // recognised even if it arrives before this thread finishes.
        let mut parent = match heads.lock() {
            Ok(mut h) => {
                h.self_published.insert(content_version.clone());
                h.head(stream)
            }
            Err(_) => GENESIS_PARENT.to_string(),
        };

        for attempt in 0..=MAX_PARENT_RETRIES {
            let body = GameEventReq {
                sender_identity: &sender_identity,
                session_token: &session_token,
                message: &message,
                content_version: &content_version,
                content_parent: &parent,
            };
            match client.put(&url).json(&body).send() {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(mut h) = heads.lock() {
                        h.set_head(stream, content_version.clone());
                    }
                    debug!("[braid-transport] published to {game_id}/{stream}");
                    return;
                }
                Ok(resp) if resp.status() == reqwest::StatusCode::CONFLICT => {
                    match resp.json::<ParentMismatchResp>() {
                        Ok(m) => {
                            debug!(
                                "[braid-transport] re-chaining {stream} publish for game {game_id} onto head {} (attempt {})",
                                m.expected_parent,
                                attempt + 1
                            );
                            if let Ok(mut h) = heads.lock() {
                                h.set_head(stream, m.expected_parent.clone());
                            }
                            parent = m.expected_parent;
                        }
                        Err(e) => {
                            warn!(
                                "[NET] Move sync backup (Braid) got an unreadable conflict for {stream} on game {game_id}: {e}"
                            );
                            return;
                        }
                    }
                }
                Ok(resp) => {
                    warn!(
                        "[NET] Move sync backup (Braid) couldn't save {stream} for game {game_id}: server returned HTTP {}",
                        resp.status()
                    );
                    return;
                }
                Err(e) => {
                    warn!(
                        "[NET] Move sync backup (Braid) couldn't save {stream} for game {game_id}: {e}"
                    );
                    return;
                }
            }
        }
        warn!(
            "[NET] Move sync backup (Braid) gave up publishing {stream} for game {game_id} after {MAX_PARENT_RETRIES} re-chain attempts"
        );
    });
}

pub fn publish_move(
    base_url: String,
    game_id: String,
    sender_identity: String,
    session_token: String,
    payload: MovePayload,
    content_version: String,
    heads: SharedStreamHeads,
) {
    publish(
        base_url,
        game_id,
        "moves",
        sender_identity,
        session_token,
        ChessMessage::Move(payload),
        content_version,
        heads,
    );
}

pub fn publish_resign(
    base_url: String,
    game_id: String,
    sender_identity: String,
    session_token: String,
    resigning_player: String,
    heads: SharedStreamHeads,
) {
    let content_version = braid_chess::version_hash(&format!("resign:{resigning_player}"), 0);
    publish(
        base_url,
        game_id,
        "moves",
        sender_identity,
        session_token,
        ChessMessage::Resign {
            player: resigning_player,
        },
        content_version,
        heads,
    );
}

pub fn publish_chat(
    base_url: String,
    game_id: String,
    sender_identity: String,
    session_token: String,
    player: String,
    text: String,
    timestamp_ms: u64,
    heads: SharedStreamHeads,
) {
    let content_version = braid_chess::version_hash(&format!("chat:{player}:{timestamp_ms}"), 0);
    publish(
        base_url,
        game_id,
        "chat",
        sender_identity,
        session_token,
        ChessMessage::Chat(ChatPayload {
            player,
            text,
            timestamp_ms,
        }),
        content_version,
        heads,
    );
}

/// SessionInfo is sent once per player, near game start. It is *not*
/// automatically genesis-parented: both players post one into the same shared
/// moves stream, so whichever lands second must chain off the first —
/// hardcoding genesis here is what used to wedge every wagered game's moves
/// stream before move one.
#[allow(clippy::too_many_arguments)]
pub fn publish_session_info(
    base_url: String,
    game_id: String,
    sender_identity: String,
    session_token: String,
    wallet_pubkey: String,
    session_pubkey: String,
    signing_pubkey: String,
    expires_at: i64,
    heads: SharedStreamHeads,
) {
    let content_version = braid_chess::version_hash(&format!("session:{wallet_pubkey}"), 0);
    publish(
        base_url,
        game_id,
        "moves",
        sender_identity,
        session_token,
        ChessMessage::SessionInfo {
            player_pubkey: wallet_pubkey,
            session_pubkey,
            signing_pubkey,
            expires_at,
        },
        content_version,
        heads,
    );
}

// ── Subscribe (with reconnect) ─────────────────────────────────────────────

/// Bridges the Braid moves+chat subscriptions for the active game into the
/// same Bevy message bus gossip feeds, with reconnect-on-drop.
#[derive(Resource, Default)]
pub struct BraidTransportState {
    /// Causal head of each stream for the active game, shared with the
    /// publish threads — see [`BraidStreamHeads`].
    heads: SharedStreamHeads,
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
            heads: SharedStreamHeads::default(),
        }
    }

    /// Handle to the shared stream heads, for passing to `publish_*`.
    pub fn heads(&self) -> SharedStreamHeads {
        self.heads.clone()
    }

    pub fn reset(&mut self) {
        self.game_id.clear();
        self.rx = None;
        self.connected
            .store(false, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut h) = self.heads.lock() {
            *h = BraidStreamHeads::default();
        }
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
        // Advance the shared stream head to whatever the server actually
        // accepted, and drop the echo of our own publishes. The backend
        // broadcasts every accepted event to all subscribers *including the
        // publisher*, so without the `self_published` check we would re-apply
        // our own moves as if the opponent had sent them, and re-display our
        // own chat lines twice.
        let mut is_self_echo = false;
        if let Some(version) = version_of(&msg) {
            if let Ok(mut h) = state.heads.lock() {
                h.set_head(stream_of(&msg), version.clone());
                is_self_echo = h.self_published.remove(&version);
            }
        }
        if is_self_echo {
            continue;
        }

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
                    signer_pubkey: Vec::new(),
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
            ChessMessage::SessionInfo {
                player_pubkey,
                session_pubkey,
                signing_pubkey,
                expires_at,
            } => {
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

                // The roster update above only feeds move-signer
                // verification. `handle_session_info_from_network`
                // (multiplayer/systems.rs) is the only place
                // `SolanaIntegrationState::opponent_pubkey` gets set —
                // required before `bridge.rs` can finalize a game
                // on-chain — and it listens only for
                // `NetworkEvent::MessageReceived(NetworkMessage::SessionInfo)`.
                // Without re-emitting that event here too, a SessionInfo
                // that only arrives via Braid (because gossip dropped it —
                // the exact failure this dual-transport send exists for)
                // never reaches that handler, and both sides get stuck
                // logging "Opponent pubkey unavailable" forever at game
                // end, never settling on-chain.
                #[cfg(feature = "solana")]
                {
                    use solana_sdk::pubkey::Pubkey;
                    use std::str::FromStr;
                    match (
                        Pubkey::from_str(&player_pubkey),
                        Pubkey::from_str(&session_pubkey),
                        Pubkey::from_str(&signing_pubkey),
                    ) {
                        (Ok(player_pubkey), Ok(session_pubkey), Ok(signing_pubkey)) => {
                            network_events.write(NetworkEvent::MessageReceived(
                                NetworkMessage::SessionInfo {
                                    game_id: game_id_u64,
                                    player_pubkey,
                                    session_pubkey,
                                    signing_pubkey,
                                    expires_at,
                                },
                            ));
                        }
                        _ => {
                            warn!(
                                "[braid-transport] SessionInfo for game {} had an unparseable pubkey — opponent_pubkey not updated via Braid fallback",
                                game_id_u64
                            );
                        }
                    }
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

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use braid_chess::message::{ChatPayload, MovePayload};

    fn move_msg(fen_after: &str, move_number: u32) -> ChessMessage {
        ChessMessage::Move(MovePayload::from_uci("e2e4", fen_after, move_number, "p"))
    }

    /// The core invariant the 409 bug violated: the moves-stream head must
    /// advance on *any* delivered move, not only on ones we published. Both
    /// players write to `/game/:id/moves`, so after the opponent moves, our
    /// next publish has to name *their* version as its parent.
    #[test]
    fn head_advances_on_opponent_moves_not_just_our_own() {
        let mut heads = BraidStreamHeads::default();
        assert_eq!(heads.head("moves"), GENESIS_PARENT);

        // Opponent's move arrives over the subscription.
        let opponent = move_msg("fen-opponent", 1);
        let v_opponent = version_of(&opponent).unwrap();
        heads.set_head(stream_of(&opponent), v_opponent.clone());

        // Our next publish must chain off it. Before the fix this still read
        // "0" (or our own last version), which the backend rejected with 409.
        assert_eq!(heads.head("moves"), v_opponent);
    }

    /// Chat and moves are validated against separate heads by the backend
    /// (`put_event`'s per-stream `head_sql`), so a chat message must not
    /// disturb the moves head or vice versa.
    #[test]
    fn move_and_chat_heads_are_independent() {
        let mut heads = BraidStreamHeads::default();

        let mv = move_msg("fen1", 1);
        heads.set_head(stream_of(&mv), version_of(&mv).unwrap());
        let moves_head = heads.head("moves");

        let chat = ChessMessage::Chat(ChatPayload {
            player: "alice".to_string(),
            text: "gg".to_string(),
            timestamp_ms: 7,
        });
        assert_eq!(stream_of(&chat), "chat");
        heads.set_head(stream_of(&chat), version_of(&chat).unwrap());

        assert_eq!(
            heads.head("moves"),
            moves_head,
            "chat must not move the moves head"
        );
        assert_ne!(heads.head("chat"), GENESIS_PARENT);
        assert_ne!(heads.head("chat"), moves_head);
    }

    /// `version_of` is what matches a *delivered* message back to what we
    /// published — it must reproduce each `publish_*` helper's `version_hash`
    /// inputs exactly, or self-echo detection and head tracking both break.
    #[test]
    fn version_of_matches_the_publish_side_hashes() {
        let mv = move_msg("fen-after", 3);
        assert_eq!(
            version_of(&mv).unwrap(),
            braid_chess::version_hash("fen-after", 3)
        );

        let resign = ChessMessage::Resign {
            player: "white".to_string(),
        };
        assert_eq!(
            version_of(&resign).unwrap(),
            braid_chess::version_hash("resign:white", 0)
        );

        let chat = ChessMessage::Chat(ChatPayload {
            player: "bob".to_string(),
            text: "hi".to_string(),
            timestamp_ms: 42,
        });
        assert_eq!(
            version_of(&chat).unwrap(),
            braid_chess::version_hash("chat:bob:42", 0)
        );

        let session = ChessMessage::SessionInfo {
            player_pubkey: "wallet1".to_string(),
            session_pubkey: "s".to_string(),
            signing_pubkey: "g".to_string(),
            expires_at: 0,
        };
        assert_eq!(
            version_of(&session).unwrap(),
            braid_chess::version_hash("session:wallet1", 0)
        );
    }

    /// The backend broadcasts accepted events to every subscriber including
    /// the publisher. Our own move coming back must be recognised and dropped,
    /// not replayed onto the board as if the opponent had sent it.
    #[test]
    fn own_publishes_are_recognised_as_echoes_exactly_once() {
        let mut heads = BraidStreamHeads::default();
        let mine = move_msg("fen-mine", 1);
        let v = version_of(&mine).unwrap();

        // publish() claims the version up-front.
        heads.self_published.insert(v.clone());

        // First delivery is our own echo.
        assert!(heads.self_published.remove(&v), "own echo must be detected");
        // A second, genuinely-remote message with the same version would not
        // be swallowed (the claim is consumed, not permanent).
        assert!(!heads.self_published.remove(&v));
    }

    /// A SessionInfo posted by the opponent must not be mistaken for our own.
    #[test]
    fn opponent_session_info_is_not_treated_as_an_echo() {
        let mut heads = BraidStreamHeads::default();
        let ours = ChessMessage::SessionInfo {
            player_pubkey: "us".to_string(),
            session_pubkey: "s".to_string(),
            signing_pubkey: "g".to_string(),
            expires_at: 0,
        };
        heads.self_published.insert(version_of(&ours).unwrap());

        let theirs = ChessMessage::SessionInfo {
            player_pubkey: "them".to_string(),
            session_pubkey: "s2".to_string(),
            signing_pubkey: "g2".to_string(),
            expires_at: 0,
        };
        let v_theirs = version_of(&theirs).unwrap();
        assert!(
            !heads.self_published.remove(&v_theirs),
            "opponent SessionInfo must be delivered, not swallowed as an echo"
        );
    }
}
