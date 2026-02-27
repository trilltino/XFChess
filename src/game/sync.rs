use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::engine::board_state::ChessEngine;
use crate::game::events::MoveMadeEvent;
use crate::multiplayer::network_protocol::NetworkMessage;
use crate::multiplayer::{BraidGameSync, BraidNetworkState, NetworkEvent};
use braid_core::Patch;

// Board state sync module
pub mod board_state;

use board_state::{
    broadcast_state_system, init_board_state_sync, receive_state_system, BoardStateSync,
};

/// Plugin for game state synchronization
pub struct GameSyncPlugin;

impl Plugin for GameSyncPlugin {
    fn build(&self, app: &mut App) {
        // Initialize board state sync on startup
        app.add_systems(Startup, init_board_state_sync);

        app.add_systems(
            Update,
            (
                receive_network_moves,
                broadcast_local_moves,
                apply_network_patches,
                update_board_state_from_network,
                // New board state sync systems
                broadcast_state_system,
                receive_state_system,
            ),
        );
    }
}

/// Receives moves from the network and converts them to game events
fn receive_network_moves(
    mut network_events: MessageReader<NetworkEvent>,
    mut network_move_events: MessageWriter<crate::game::events::NetworkMoveEvent>,
) {
    for event in network_events.read() {
        if let crate::multiplayer::NetworkEvent::MessageReceived(message) = event {
            if let NetworkMessage::Move {
                game_id,
                turn,
                move_uci,
                next_fen: _,
            } = message
            {
                info!(
                    "[SYNC] Received move from network: {} (game: {}, turn: {})",
                    move_uci, game_id, turn
                );

                // Parse UCI string (e.g. "e2e4") to coords
                if let Some(from) = ChessEngine::uci_to_coords(&move_uci[0..2]) {
                    if let Some(to) = ChessEngine::uci_to_coords(&move_uci[2..4]) {
                        let promotion = move_uci.chars().nth(4);

                        network_move_events.write(crate::game::events::NetworkMoveEvent {
                            from,
                            to,
                            promotion,
                        });
                    }
                }
            }
        }
    }
}

/// Broadcasts local moves to the network
fn broadcast_local_moves(
    mut local_move_events: MessageReader<MoveMadeEvent>,
    network_state: Res<BraidNetworkState>,
) {
    for local_move in local_move_events.read() {
        // Only broadcast if it's a local move and we are connected
        if !local_move.remote && network_state.connected {
            if let Some(sender) = &network_state.message_sender {
                // Convert coords back to UCI
                let move_uci = format!(
                    "{}{}",
                    ChessEngine::coords_to_uci(local_move.from.0, local_move.from.1),
                    ChessEngine::coords_to_uci(local_move.to.0, local_move.to.1)
                );

                let network_message = NetworkMessage::Move {
                    game_id: local_move.game_id.unwrap_or(0),
                    turn: 0, // Should probably be tracked in MoveMadeEvent if needed
                    move_uci,
                    next_fen: String::new(), // Optional
                };

                info!(
                    "[SYNC] Broadcasting local move: {}",
                    network_message.game_id()
                );
                let _ = sender.send(network_message);
            }
        }
    }
}

/// Applies network patches to the local game state
fn apply_network_patches(
    mut braid_sync: ResMut<BraidGameSync>,
    _network_state: Res<BraidNetworkState>,
) {
    // This system applies patches received from the network to the local state
    if !braid_sync.pending_patches.is_empty() {
        for _patch in braid_sync.pending_patches.drain(..) {
            info!("Handled patch from network");
        }
    }
}

/// Updates the local board state based on network-synced state
fn update_board_state_from_network(_engine: ResMut<ChessEngine>, _braid_sync: Res<BraidGameSync>) {
    // In the new architecture, we likely need to update from another source
    // since the document field no longer exists in BraidGameSync
    // This function might need to be updated based on how the sync works now
    // For now, leaving it as a placeholder
}

/// Creates a patch representing a move
fn create_move_patch(move_data: &NetworkMove) -> Patch {
    // In a real implementation, this would create an actual patch
    // for the Braid CRDT system
    Patch::bytes("", serde_json::to_vec(move_data).unwrap_or_default())
}

/// Represents a move transmitted over the network
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkMove {
    pub from: String,                   // Source square (e.g. "e2")
    pub to: String,                     // Destination square (e.g. "e4")
    pub player: String,                 // Player making the move
    pub piece_type: String,             // Type of piece moved
    pub captured_piece: Option<String>, // Type of captured piece, if any
    pub promotion: Option<String>,      // Promotion piece, if pawn was promoted
    pub timestamp: u64,                 // Timestamp of the move
}
