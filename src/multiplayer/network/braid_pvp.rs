//! Braid PvP transport — iroh gossip only.
//!
//! Moves, clock snapshots, and chat now travel exclusively over iroh gossip
//! (`NetworkMessage` variants). The old Braid-HTTP subscriber/publisher tasks
//! have been removed; the only remaining HTTP usage is the VPS `record_move`
//! REST call (canonical on-chain record, separate from the P2P layer).
//!
//! What this module still does:
//! - Owns `BraidPvpSession` (session config + move counter + nonce)
//! - Fires `PublishBraidResign` / `PublishBraidChat` events → gossip
//! - Broadcasts `NetworkMessage::Clock` after each local move
//! - Broadcasts `NetworkMessage::Chat` for outgoing messages
//! - Drains incoming `NetworkMessage::Chat` / `NetworkMessage::Clock` from gossip
//!   into Bevy events for UI consumers
//! - Sends `GameSnapshot` when a new peer joins mid-game

#![allow(dead_code)]

use bevy::prelude::*;
use braid_uri::MovePayload;
use tracing::{info, warn};

use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::types::NetworkEvent;

/// Configuration + runtime state for a single PvP game.
#[derive(Resource)]
pub struct BraidPvpSession {
    /// Base URL of the VPS backend (e.g. `http://127.0.0.1:8090`).
    /// Used only for VPS `record_move` — never for live P2P traffic.
    pub base_url: String,
    /// Posted game identifier.
    pub game_id: String,
    /// Whether the session is active.
    pub active: bool,
    /// Incrementing move counter for [`MovePayload::move_number`].
    pub next_move_number: u32,
    /// Next nonce for on-chain replay protection.
    pub next_nonce: u64,
    /// Wager amount in SOL (0 = casual).
    pub wager_amount: f64,
    /// Content-addressed version of the last move we published.
    /// Used to populate `GameSnapshot::head_version` for catch-up.
    pub last_version: String,
}

impl Default for BraidPvpSession {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            game_id: String::new(),
            active: false,
            next_move_number: 1,
            next_nonce: 1,
            wager_amount: 0.0,
            last_version: "0".to_string(),
        }
    }
}

impl BraidPvpSession {
    pub fn reset(&mut self) {
        self.base_url.clear();
        self.game_id.clear();
        self.active = false;
        self.next_move_number = 1;
        self.next_nonce = 1;
        self.wager_amount = 0.0;
        self.last_version = "0".to_string();
    }

    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty() && !self.game_id.is_empty()
    }
}

/// Bevy event to publish a resign over iroh gossip.
#[derive(Message, Debug, Clone)]
pub struct PublishBraidResign {
    pub player: String,
}

/// Bevy event to publish a chat message over iroh gossip.
#[derive(Message, Debug, Clone)]
pub struct PublishBraidChat {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
}

/// Bevy event emitted when an inbound chat message arrives from the peer.
#[derive(Message, Debug, Clone)]
pub struct BraidChatMessage {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
}

/// Plugin registering the PvP Braid session resource, events, and systems.
pub struct BraidPvpPlugin;

impl Plugin for BraidPvpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BraidPvpSession>()
            .add_message::<PublishBraidResign>()
            .add_message::<PublishBraidChat>()
            .add_message::<BraidChatMessage>()
            .add_systems(
                Update,
                (
                    publish_local_move,
                    handle_publish_resign,
                    handle_publish_chat,
                    drain_chat_messages,
                    drain_clock_to_spectator,
                    publish_clock_on_move,
                ),
            );
        #[cfg(feature = "solana")]
        app.add_systems(Update, broadcast_snapshot_to_new_peer);
    }
}

/// Read `MoveMadeEvent`s, send them as `NetworkMessage::Move` over iroh gossip,
/// and update the session's causal version chain. This replaces the old
/// `publish_local_moves_via_braid` + `handle_publish_move` pair.
fn publish_local_move(
    mut session: ResMut<BraidPvpSession>,
    mut move_events: MessageReader<crate::game::events::MoveMadeEvent>,
    network_state: Res<crate::multiplayer::BraidNetworkState>,
    p2p_conn: Res<crate::multiplayer::network::p2p::P2PConnectionState>,
) {
    if !session.is_configured() {
        move_events.clear();
        return;
    }
    for event in move_events.read() {
        if event.remote {
            continue;
        }
        let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
        let move_number = session.next_move_number;
        let nonce = session.next_nonce;
        session.next_move_number = session.next_move_number.saturating_add(1);
        session.next_nonce = session.next_nonce.saturating_add(1);

        // Build UCI string.
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

        // Causal chain fields.
        let parent_version = session.last_version.clone();
        let new_version = braid_uri::version_hash(&event.next_fen, move_number);
        session.last_version = new_version;

        // agent_id = iroh node's public key bytes (stable identity).
        let agent_id = network_state
            .node_id
            .as_ref()
            .map(|id| id.as_bytes().to_vec())
            .unwrap_or_default();

        // Use move_number as the per-agent sequence number (monotonic, unique).
        let seq = move_number as u64;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let _ = p2p_conn.player_color; // referenced for completeness; color is in the FEN

        if let Some(ref tx) = network_state.message_sender {
            let msg = NetworkMessage::Move {
                game_id: game_id_u64,
                turn: move_number as u16,
                move_uci: uci.clone(),
                next_fen: event.next_fen.clone(),
                nonce,
                timestamp_ms,
                agent_id,
                seq,
                parent_version,
            };
            if let Err(e) = tx.send(msg) {
                warn!("[braid-pvp] Move gossip send failed (game {game_id_u64}, turn {move_number}): {e}");
            } else {
                info!("[braid-pvp] Move sent via gossip (game {game_id_u64}, turn {move_number}, uci {uci})");
            }
        }
    }
}

/// Activate a fresh Braid PvP session for `game_id` against `base_url` and
/// subscribe to the corresponding iroh gossip topic. Idempotent.
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

    if let Some(ref sub_tx) = network_state.subscription_sender {
        let topic = format!("{}/{}", crate::multiplayer::systems::GAME_TOPIC, game_id);
        if let Err(e) = sub_tx.send(topic) {
            warn!("[braid-pvp] Failed to subscribe to iroh topic: {e}");
        } else {
            info!("[braid-pvp] Subscribed to iroh gossip for game {game_id}");
        }
    }
}

/// Bevy system: send `PublishBraidResign` events over iroh gossip.
fn handle_publish_resign(
    mut session: ResMut<BraidPvpSession>,
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
        let nonce = session.next_nonce;
        session.next_nonce = session.next_nonce.saturating_add(1);

        if let Some(ref tx) = network_state.message_sender {
            let msg = NetworkMessage::Resign {
                game_id: game_id_u64,
                winner: winner.to_string(),
                nonce,
            };
            if let Err(e) = tx.send(msg) {
                warn!("[braid-pvp] Resign gossip send failed: {e}");
            } else {
                info!("[braid-pvp] Resign sent via gossip (game {game_id_u64})");
            }
        }
    }
}

/// Bevy system: send `PublishBraidChat` events as `NetworkMessage::Chat` over gossip.
fn handle_publish_chat(
    session: Res<BraidPvpSession>,
    mut reader: MessageReader<PublishBraidChat>,
    network_state: Res<crate::multiplayer::BraidNetworkState>,
) {
    if !session.is_configured() {
        reader.clear();
        return;
    }
    for event in reader.read() {
        let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
        if let Some(ref tx) = network_state.message_sender {
            let msg = NetworkMessage::Chat {
                game_id: game_id_u64,
                player: event.player.clone(),
                text: event.text.clone(),
                timestamp_ms: event.timestamp_ms,
            };
            if let Err(e) = tx.send(msg) {
                warn!("[braid-pvp] Chat gossip send failed: {e}");
            }
        }
    }
}

/// Drain incoming `NetworkMessage::Chat` events into `BraidChatMessage` Bevy events.
fn drain_chat_messages(
    session: Res<BraidPvpSession>,
    mut network_events: MessageReader<NetworkEvent>,
    mut writer: MessageWriter<BraidChatMessage>,
) {
    if !session.is_configured() {
        return;
    }
    let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
    for ev in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::Chat {
            game_id,
            player,
            text,
            timestamp_ms,
        }) = ev
        {
            if *game_id == game_id_u64 {
                writer.write(BraidChatMessage {
                    player: player.clone(),
                    text: text.clone(),
                    timestamp_ms: *timestamp_ms,
                });
            }
        }
    }
}

/// Drain incoming `NetworkMessage::Clock` events into `SpectatorClockState`.
fn drain_clock_to_spectator(
    session: Res<BraidPvpSession>,
    mut network_events: MessageReader<NetworkEvent>,
    mut clock: ResMut<crate::multiplayer::spectator::SpectatorClockState>,
    time: Res<Time>,
) {
    if !session.is_configured() {
        return;
    }
    let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
    for ev in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::Clock {
            game_id,
            white_ms,
            black_ms,
            ..
        }) = ev
        {
            if *game_id == game_id_u64 {
                clock.white_ms = *white_ms;
                clock.black_ms = *black_ms;
                clock.last_update_secs = time.elapsed_secs_f64();
            }
        }
    }
}

/// Broadcast `NetworkMessage::Clock` over gossip after each local move.
fn publish_clock_on_move(
    mut move_events: MessageReader<crate::game::events::MoveMadeEvent>,
    session: Res<BraidPvpSession>,
    game_timer: Option<Res<crate::game::resources::turn::timer::GameTimer>>,
    network_state: Res<crate::multiplayer::BraidNetworkState>,
) {
    if !session.is_configured() {
        move_events.clear();
        return;
    }
    let Some(timer) = game_timer else {
        move_events.clear();
        return;
    };

    for event in move_events.read() {
        if event.remote {
            continue;
        }
        let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
        let white_ms = (timer.white_time_left * 1000.0) as u64;
        let black_ms = (timer.black_time_left * 1000.0) as u64;
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        if let Some(ref tx) = network_state.message_sender {
            let _ = tx.send(NetworkMessage::Clock {
                game_id: game_id_u64,
                white_ms,
                black_ms,
                timestamp_ms,
            });
        }
    }
}

/// When a new peer joins the gossip topic mid-game, broadcast a `GameSnapshot`
/// so they can catch up immediately.
#[cfg(feature = "solana")]
fn broadcast_snapshot_to_new_peer(
    session: Res<BraidPvpSession>,
    mut network_events: MessageReader<NetworkEvent>,
    network_state: Res<crate::multiplayer::BraidNetworkState>,
    rollup_manager: Res<crate::multiplayer::EphemeralRollupManager>,
) {
    if !session.is_configured() || session.last_version == "0" {
        network_events.clear();
        return;
    }
    let game_id = match session.game_id.parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            network_events.clear();
            return;
        }
    };

    let mut peer_joined = false;
    for event in network_events.read() {
        if let NetworkEvent::PeerConnected(_) = event {
            peer_joined = true;
        }
    }
    if !peer_joined {
        return;
    }

    let committed_fen = rollup_manager.committed_fen.clone();
    let head_version = session.last_version.clone();
    let msg_tx = network_state.message_sender.clone();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            use crate::multiplayer::vps_client;
            let moves = vps_client::fetch_move_log(game_id).unwrap_or_default();
            let move_payloads: Vec<String> = moves
                .iter()
                .filter_map(|m| serde_json::to_string(m).ok())
                .collect();

            info!(
                "[braid-pvp] New peer joined — sending GameSnapshot for game {} ({} moves)",
                game_id,
                move_payloads.len()
            );

            if let Some(tx) = msg_tx {
                let _ = tx.send(NetworkMessage::GameSnapshot {
                    game_id,
                    fen: committed_fen,
                    move_payloads,
                    head_version,
                });
            }
        })
        .detach();
}

/// Update `last_version` on the session after a local move is published via
/// gossip (called by the game's `handle_publish_move` equivalent in systems.rs).
/// This keeps the catch-up snapshot's `head_version` accurate.
pub fn advance_session_version(session: &mut BraidPvpSession, fen_after: &str) {
    let version = braid_uri::version_hash(fen_after, session.next_move_number.saturating_sub(1));
    session.last_version = version;
}

/// Build a `MovePayload` for the VPS `record_move` REST call.
/// Does NOT send anything over the network — call site handles the HTTP.
pub fn make_move_payload(
    session: &BraidPvpSession,
    uci: String,
    fen_after: String,
    player: String,
) -> MovePayload {
    MovePayload::from_uci(uci, fen_after, session.next_move_number.saturating_sub(1), player)
}
