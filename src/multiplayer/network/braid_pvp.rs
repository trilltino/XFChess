//! Braid-HTTP PvP transport built on the `braid_uri` crate.
//!
//! This module wires the typed [`braid_uri`] publisher/subscriber into the
//! Bevy multiplayer stack so posted PvP games (and, optionally, non-Swiss
//! match streams inside tournaments) can push and receive moves over the
//! Braid-HTTP resource protocol.
//!
//! The transport layer is orthogonal to the existing VPS HTTP relay and to
//! the braid-iroh gossip used for tournament metadata. It is driven by a
//! single Bevy [`Resource`] ([`BraidPvpSession`]) which, when activated,
//! spawns a background subscriber that forwards incoming [`ChessMessage`]
//! events onto a [`crossbeam_channel`] consumed from a Bevy [`Update`] system.

#![allow(dead_code)]

use bevy::prelude::*;
use braid_uri::{ChatPayload, ChessMessage, ChessPublisher, ChessSubscriber, MovePayload};
use crossbeam_channel::{bounded, Receiver, Sender};
use tracing::{error, info, warn};

/// Configuration + runtime handle for a single posted PvP game's Braid stream.
///
/// Only one PvP session is active at a time per client, so this is modeled
/// as a single Bevy [`Resource`]. Set `active = true` and populate the URL
/// + game id to kick off a background subscriber task.
#[derive(Resource)]
pub struct BraidPvpSession {
    /// Base URL of the Braid-HTTP backend (e.g. `http://127.0.0.1:8090`).
    pub base_url: String,
    /// Posted game identifier (shared between host and joiner).
    pub game_id: String,
    /// Whether the subscriber task should be running.
    pub active: bool,
    /// Channel receiver for decoded inbound chess messages.
    pub rx: Receiver<ChessMessage>,
    /// Internal sender paired with `rx` — handed to the background task.
    sender: Sender<ChessMessage>,
    /// Inbound chat messages from the peer.
    pub chat_rx: Receiver<ChatPayload>,
    /// Internal sender for the chat channel — handed to the background chat subscriber task.
    chat_sender: Sender<ChatPayload>,
    /// Incrementing move counter used to populate [`MovePayload::move_number`].
    pub next_move_number: u32,
    /// Next nonce for on-chain replay protection (must match Game.nonce + 1).
    pub next_nonce: u64,
    /// Wager amount in SOL (0 = casual unranked, >0 = ranked/wager PvP with ER settlement).
    pub wager_amount: f64,
}

impl Default for BraidPvpSession {
    fn default() -> Self {
        let (tx, rx) = bounded(256);
        let (chat_tx, chat_rx) = bounded(64);
        Self {
            base_url: String::new(),
            game_id: String::new(),
            active: false,
            rx,
            sender: tx,
            chat_rx,
            chat_sender: chat_tx,
            next_move_number: 1,
            next_nonce: 1,
            wager_amount: 0.0,
        }
    }
}

impl BraidPvpSession {
    /// Reset the session (drops both halves of the channel and starts fresh).
    pub fn reset(&mut self) {
        let (tx, rx) = bounded(256);
        let (chat_tx, chat_rx) = bounded(64);
        self.sender = tx;
        self.rx = rx;
        self.chat_sender = chat_tx;
        self.chat_rx = chat_rx;
        self.base_url.clear();
        self.game_id.clear();
        self.active = false;
        self.next_move_number = 1;
        self.next_nonce = 1;
        self.wager_amount = 0.0;
    }

    /// Whether the session has enough info to talk to the backend.
    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty() && !self.game_id.is_empty()
    }
}

/// Bevy event emitted when the Braid subscriber receives an inbound message.
#[derive(Message, Debug, Clone)]
pub struct BraidPvpIncomingMessage(pub ChessMessage);

/// Bevy event that callers fire to request publishing a move via Braid-HTTP.
#[derive(Message, Debug, Clone)]
pub struct PublishBraidMove {
    pub uci: String,
    pub fen_after: String,
    pub player: String,
}

/// Bevy event to publish a resign over the Braid move stream.
#[derive(Message, Debug, Clone)]
pub struct PublishBraidResign {
    pub player: String,
}

/// Bevy event to publish a chat message over the Braid chat stream.
#[derive(Message, Debug, Clone)]
pub struct PublishBraidChat {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
}

/// Plugin registering the PvP Braid session resource, events, and systems.
pub struct BraidPvpPlugin;

impl Plugin for BraidPvpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BraidPvpSession>()
            .add_message::<BraidPvpIncomingMessage>()
            .add_message::<PublishBraidMove>()
            .add_message::<PublishBraidResign>()
            .add_message::<PublishBraidChat>()
            .add_systems(
                Update,
                (drain_incoming_messages, handle_publish_move, handle_publish_resign, handle_publish_chat, publish_local_moves_via_braid),
            );
    }
}

/// Activate a fresh Braid PvP session for `game_id` against `base_url` and
/// spawn the subscriber background task. Idempotent — calling it twice with
/// the same config is a no-op; calling it with different config resets first.
/// Also subscribes to the Iroh gossip topic for this game if the network layer is up.
///
/// When `wager_amount > 0`, the game will be settled via MagicBlock Ephemeral Rollups
/// (delegation → batch commit → undelegate + finalize on game end).
pub fn start_session(
    session: &mut BraidPvpSession,
    base_url: String,
    game_id: String,
    wager_amount: f64,
    network_state: &crate::multiplayer::BraidNetworkState,
) {
    if session.active && session.base_url == base_url && session.game_id == game_id {
        return;
    }
    session.reset();
    session.base_url = base_url.clone();
    session.game_id = game_id.clone();
    session.wager_amount = wager_amount;
    session.active = true;

    // ── Subscribe to Iroh gossip topic for this game (fast path) ──────────
    if let Some(ref sub_tx) = network_state.subscription_sender {
        let topic = format!("{}/{}", crate::multiplayer::systems::GAME_TOPIC, game_id);
        if let Err(e) = sub_tx.send(topic) {
            warn!("[braid-pvp] Failed to send Iroh subscription request: {e}");
        } else {
            info!("[braid-pvp] Subscribed to Iroh gossip for game {game_id}");
        }
    }

    let tx = session.sender.clone();
    let chat_tx = session.chat_sender.clone();
    let base_url_chat = base_url.clone();
    let game_id_chat = game_id.clone();

    // ── Moves subscriber ──────────────────────────────────────────────────────
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let sub = match ChessSubscriber::new(&base_url, &game_id) {
                Ok(s) => s,
                Err(e) => {
                    error!("[braid-pvp] Failed to build Braid-HTTP subscriber: {e}");
                    return;
                }
            };
            let (rx, _handle) = match sub.subscribe_moves().await {
                Ok(x) => x,
                Err(e) => {
                    error!("[braid-pvp] Braid-HTTP subscribe_moves failed: {e}");
                    return;
                }
            };
            info!(
                "[braid-pvp] Subscribed to Braid-HTTP moves stream for game {} @ {}",
                game_id, base_url
            );
            while let Ok(msg) = rx.recv().await {
                if tx.send(msg).is_err() {
                    info!("[braid-pvp] Bevy-side receiver dropped — stopping Braid-HTTP subscriber");
                    break;
                }
            }
        })
        .detach();

    // ── Chat subscriber ───────────────────────────────────────────────────────
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let sub = match ChessSubscriber::new(&base_url_chat, &game_id_chat) {
                Ok(s) => s,
                Err(e) => {
                    error!("[braid-pvp] Failed to build Braid-HTTP chat subscriber: {e}");
                    return;
                }
            };
            let (rx, _handle) = match sub.subscribe_chat().await {
                Ok(x) => x,
                Err(e) => {
                    error!("[braid-pvp] Braid-HTTP subscribe_chat failed: {e}");
                    return;
                }
            };
            info!(
                "[braid-pvp] Subscribed to Braid-HTTP chat stream for game {} @ {}",
                game_id_chat, base_url_chat
            );
            while let Ok(msg) = rx.recv().await {
                if let ChessMessage::Chat(payload) = msg {
                    if chat_tx.send(payload).is_err() {
                        break;
                    }
                }
            }
        })
        .detach();
}

/// Bevy system: drain any queued inbound messages into [`BraidPvpIncomingMessage`] events
/// and also inject them into the main [`NetworkEvent`] stream for the game loop.
fn drain_incoming_messages(
    session: Res<BraidPvpSession>,
    mut writer: MessageWriter<BraidPvpIncomingMessage>,
    mut network_events: MessageWriter<crate::multiplayer::types::NetworkEvent>,
) {
    if !session.active {
        return;
    }
    
    // Parse game_id as u64 once for this frame
    let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);

    // Use try_recv to avoid blocking the main thread.
    while let Ok(msg) = session.rx.try_recv() {
        writer.write(BraidPvpIncomingMessage(msg.clone()));

        // Translate Braid-specific ChessMessage to generic NetworkMessage
        let net_msg = match msg {
            ChessMessage::Move(p) => Some(crate::multiplayer::network::protocol::NetworkMessage::Move {
                game_id: game_id_u64,
                turn: p.move_number as u16,
                move_uci: p.uci,
                next_fen: p.fen_after,
                // Use move_number as the nonce so the replay-protection check in
                // handle_network_events accepts Braid-HTTP moves in sequence.
                nonce: p.move_number as u64,
                timestamp_ms: 0,
            }),
            ChessMessage::Resign { player } => Some(crate::multiplayer::network::protocol::NetworkMessage::Resign {
                game_id: game_id_u64,
                winner: if player == "white" { "black".to_string() } else { "white".to_string() },
                // Resign carries a single-use nonce; use a large sentinel so it's never
                // confused with a move nonce and always passes the replay check.
                nonce: u64::MAX,
            }),
            _ => None,
        };

        if let Some(m) = net_msg {
            network_events.write(crate::multiplayer::types::NetworkEvent::MessageReceived(m));
        }
    }
}

/// Bevy system: react to [`PublishBraidMove`] by spawning a publish task.
/// Publishes over both Iroh gossip (fast path) and Braid-HTTP (fallback).
fn handle_publish_move(
    mut session: ResMut<BraidPvpSession>,
    mut reader: MessageReader<PublishBraidMove>,
    network_state: Res<crate::multiplayer::BraidNetworkState>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
) {
    if !session.is_configured() {
        reader.clear();
        return;
    }
    for event in reader.read() {
        let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
        let move_number = session.next_move_number;
        let nonce = session.next_nonce;
        session.next_move_number = session.next_move_number.saturating_add(1);
        session.next_nonce = session.next_nonce.saturating_add(1);

        // ── Fast path: Iroh gossip (fire-and-forget) ──────────────────────────
        if let Some(ref tx) = network_state.message_sender {
            let timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let iroh_msg = crate::multiplayer::network::protocol::NetworkMessage::Move {
                game_id: game_id_u64,
                turn: move_number as u16,
                move_uci: event.uci.clone(),
                next_fen: event.fen_after.clone(),
                nonce,
                timestamp_ms,
            };
            if let Err(e) = tx.send(iroh_msg) {
                warn!("[braid-pvp] Iroh fast-path publish failed (tx closed): {e}");
            } else {
                info!("[braid-pvp] Move sent via Iroh gossip (game {game_id_u64}, turn {}, nonce {})", move_number, nonce);
            }
        }

        // ── Fallback / redundant path: Braid-HTTP ───────────────────────────
        let base = session.base_url.clone();
        let game = session.game_id.clone();
        let payload = MovePayload::from_uci(
            event.uci.clone(),
            event.fen_after.clone(),
            move_number,
            event.player.clone(),
        );
        spawn_publish_move(base, game, payload, tokio_runtime.0.handle().clone());
    }
}

/// Bevy system: react to [`PublishBraidResign`] by spawning a publish task.
/// Publishes over both Iroh gossip and Braid-HTTP.
fn handle_publish_resign(
    mut session: ResMut<BraidPvpSession>,
    mut reader: MessageReader<PublishBraidResign>,
    network_state: Res<crate::multiplayer::BraidNetworkState>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
) {
    if !session.is_configured() {
        reader.clear();
        return;
    }
    for event in reader.read() {
        let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
        let winner = if event.player == "white" { "black" } else { "white" };
        let nonce = session.next_nonce;
        session.next_nonce = session.next_nonce.saturating_add(1);

        // ── Fast path: Iroh gossip ────────────────────────────────────────────
        if let Some(ref tx) = network_state.message_sender {
            let iroh_msg = crate::multiplayer::network::protocol::NetworkMessage::Resign {
                game_id: game_id_u64,
                winner: winner.to_string(),
                nonce,
            };
            if let Err(e) = tx.send(iroh_msg) {
                warn!("[braid-pvp] Iroh resign publish failed (tx closed): {e}");
            } else {
                info!("[braid-pvp] Resign sent via Iroh gossip (game {game_id_u64}, nonce {})", nonce);
            }
        }

        // ── Fallback / redundant path: Braid-HTTP ───────────────────────────
        spawn_publish_resign(
            session.base_url.clone(),
            session.game_id.clone(),
            event.player.clone(),
            tokio_runtime.0.handle().clone(),
        );
    }
}

/// Forward local `MoveMadeEvent`s to the Braid-HTTP publish stream via `PublishBraidMove`.
/// Only fires in BraidMultiplayer mode when the session is configured.
fn publish_local_moves_via_braid(
    mut move_events: MessageReader<crate::game::events::MoveMadeEvent>,
    mut publish_events: MessageWriter<PublishBraidMove>,
    game_mode: Res<crate::core::states::GameMode>,
    session: Res<BraidPvpSession>,
    p2p_conn: Res<crate::multiplayer::network::p2p::P2PConnectionState>,
) {
    if *game_mode != crate::core::states::GameMode::BraidMultiplayer || !session.is_configured() {
        return;
    }
    for event in move_events.read() {
        if event.remote {
            continue;
        }
        let from_file = (b'a' + event.from.0) as char;
        let from_rank = event.from.1 + 1;
        let to_file = (b'a' + event.to.0) as char;
        let to_rank = event.to.1 + 1;
        let mut uci = format!("{}{}{}{}", from_file, from_rank, to_file, to_rank);
        if let Some(promo) = event.promotion {
            let promo_char = match promo {
                crate::game::components::PieceType::Queen => 'q',
                crate::game::components::PieceType::Rook => 'r',
                crate::game::components::PieceType::Bishop => 'b',
                crate::game::components::PieceType::Knight => 'n',
                _ => 'q',
            };
            uci.push(promo_char);
        }
        let player = match p2p_conn.player_color {
            Some(crate::rendering::pieces::PieceColor::White) => "white",
            Some(crate::rendering::pieces::PieceColor::Black) => "black",
            None => "white",
        };
        info!("[braid-pvp] Forwarding local move {} to Braid-HTTP publish", uci);
        publish_events.write(PublishBraidMove {
            uci,
            fen_after: event.next_fen.clone(),
            player: player.to_string(),
        });
    }
}

fn spawn_publish_move(base_url: String, game_id: String, payload: MovePayload, handle: tokio::runtime::Handle) {
    handle.spawn(async move {
        let mut publisher = match ChessPublisher::new(&base_url, &game_id) {
            Ok(p) => p,
            Err(e) => {
                warn!("[braid-pvp] Braid-HTTP publisher init failed: {e}");
                return;
            }
        };
        if let Err(e) = publisher.publish_move(&payload).await {
            warn!("[braid-pvp] Braid-HTTP publish_move failed: {e}");
        }
    });
}

fn spawn_publish_resign(base_url: String, game_id: String, player: String, handle: tokio::runtime::Handle) {
    handle.spawn(async move {
        let mut publisher = match ChessPublisher::new(&base_url, &game_id) {
            Ok(p) => p,
            Err(e) => {
                warn!("[braid-pvp] Braid-HTTP publisher init failed: {e}");
                return;
            }
        };
        if let Err(e) = publisher.publish_resign(&player).await {
            warn!("[braid-pvp] Braid-HTTP publish_resign failed: {e}");
        }
    });
}

/// Bevy system: react to [`PublishBraidChat`] events — sends via Braid-HTTP chat resource.
fn handle_publish_chat(
    session: Res<BraidPvpSession>,
    mut reader: MessageReader<PublishBraidChat>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
) {
    if !session.is_configured() {
        reader.clear();
        return;
    }
    for event in reader.read() {
        let base = session.base_url.clone();
        let game = session.game_id.clone();
        let player = event.player.clone();
        let text = event.text.clone();
        let ts = event.timestamp_ms;
        tokio_runtime.0.handle().spawn(async move {
            let mut publisher = match ChessPublisher::new(&base, &game) {
                Ok(p) => p,
                Err(e) => {
                    warn!("[braid-pvp] chat publisher init failed: {e}");
                    return;
                }
            };
            if let Err(e) = publisher.publish_chat(&player, &text, ts).await {
                warn!("[braid-pvp] publish_chat failed: {e}");
            }
        });
    }
}
