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
use braid_uri::{ChessMessage, ChessPublisher, ChessSubscriber, MovePayload};
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
    /// Incrementing move counter used to populate [`MovePayload::move_number`].
    pub next_move_number: u32,
    /// Wager amount in SOL (0 = casual unranked, >0 = ranked/wager PvP with ER settlement).
    pub wager_amount: f64,
}

impl Default for BraidPvpSession {
    fn default() -> Self {
        let (tx, rx) = bounded(256);
        Self {
            base_url: String::new(),
            game_id: String::new(),
            active: false,
            rx,
            sender: tx,
            next_move_number: 1,
            wager_amount: 0.0,
        }
    }
}

impl BraidPvpSession {
    /// Reset the session (drops both halves of the channel and starts fresh).
    pub fn reset(&mut self) {
        let (tx, rx) = bounded(256);
        self.sender = tx;
        self.rx = rx;
        self.base_url.clear();
        self.game_id.clear();
        self.active = false;
        self.next_move_number = 1;
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

/// Plugin registering the PvP Braid session resource, events, and systems.
pub struct BraidPvpPlugin;

impl Plugin for BraidPvpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BraidPvpSession>()
            .add_message::<BraidPvpIncomingMessage>()
            .add_message::<PublishBraidMove>()
            .add_message::<PublishBraidResign>()
            .add_systems(
                Update,
                (drain_incoming_messages, handle_publish_move, handle_publish_resign),
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
            }),
            ChessMessage::Resign { player } => Some(crate::multiplayer::network::protocol::NetworkMessage::Resign {
                game_id: game_id_u64,
                winner: if player == "white" { "black".to_string() } else { "white".to_string() },
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
) {
    if !session.is_configured() {
        reader.clear();
        return;
    }
    for event in reader.read() {
        let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
        let move_number = session.next_move_number;
        session.next_move_number = session.next_move_number.saturating_add(1);

        // ── Fast path: Iroh gossip (fire-and-forget) ──────────────────────────
        if let Some(ref tx) = network_state.message_sender {
            let iroh_msg = crate::multiplayer::network::protocol::NetworkMessage::Move {
                game_id: game_id_u64,
                turn: move_number as u16,
                move_uci: event.uci.clone(),
                next_fen: event.fen_after.clone(),
            };
            if let Err(e) = tx.send(iroh_msg) {
                warn!("[braid-pvp] Iroh fast-path publish failed (tx closed): {e}");
            } else {
                info!("[braid-pvp] Move sent via Iroh gossip (game {game_id_u64}, turn {})", move_number);
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
        spawn_publish_move(base, game, payload);
    }
}

/// Bevy system: react to [`PublishBraidResign`] by spawning a publish task.
/// Publishes over both Iroh gossip and Braid-HTTP.
fn handle_publish_resign(
    session: Res<BraidPvpSession>,
    mut reader: MessageReader<PublishBraidResign>,
    network_state: Res<crate::multiplayer::BraidNetworkState>,
) {
    if !session.is_configured() {
        reader.clear();
        return;
    }
    for event in reader.read() {
        let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
        let winner = if event.player == "white" { "black" } else { "white" };

        // ── Fast path: Iroh gossip ────────────────────────────────────────────
        if let Some(ref tx) = network_state.message_sender {
            let iroh_msg = crate::multiplayer::network::protocol::NetworkMessage::Resign {
                game_id: game_id_u64,
                winner: winner.to_string(),
            };
            if let Err(e) = tx.send(iroh_msg) {
                warn!("[braid-pvp] Iroh resign publish failed (tx closed): {e}");
            } else {
                info!("[braid-pvp] Resign sent via Iroh gossip (game {game_id_u64})");
            }
        }

        // ── Fallback / redundant path: Braid-HTTP ───────────────────────────
        spawn_publish_resign(
            session.base_url.clone(),
            session.game_id.clone(),
            event.player.clone(),
        );
    }
}

fn spawn_publish_move(base_url: String, game_id: String, payload: MovePayload) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
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
        })
        .detach();
}

fn spawn_publish_resign(base_url: String, game_id: String, player: String) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
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
        })
        .detach();
}
