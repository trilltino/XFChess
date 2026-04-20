//! Spectator feed system - broadcasts game state to non-participants

use bevy::prelude::*;
use crate::multiplayer::{BraidNetworkState, NetworkMessage, NodeRole, NetworkEvent, GameSession};
use std::collections::HashMap;
use bincode;
use zstd::bulk::{compress, decompress};

/// Compression level for spectator messages
const COMPRESSION_LEVEL: i32 = 3;

/// Resource tracking spectator subscriptions
#[derive(Resource, Default)]
pub struct SpectatorRegistry {
    /// Map of topic -> spectator node IDs
    spectators: HashMap<String, Vec<String>>,
}

impl SpectatorRegistry {
    /// Add a spectator to a topic
    pub fn add_spectator(&mut self, topic: &str, node_id: String) {
        self.spectators
            .entry(topic.to_string())
            .or_default()
            .push(node_id);
    }

    /// Remove a spectator from all topics
    pub fn remove_spectator(&mut self, node_id: &str) {
        for spectators in self.spectators.values_mut() {
            spectators.retain(|id| id != node_id);
        }
    }

    /// Get spectators for a topic
    pub fn get_spectators(&self, topic: &str) -> Vec<String> {
        self.spectators.get(topic).cloned().unwrap_or_default()
    }
}

/// System that broadcasts game state to spectators
pub fn broadcast_game_state(
    network_state: Res<BraidNetworkState>,
    spectator_registry: Res<SpectatorRegistry>,
) {
    let Some(game_id) = network_state.active_session.as_ref().map(|s| s.session_id.clone()) else {
        return;
    };

    let Some(game_state) = network_state.active_session.as_ref().and_then(|s| s.game_state.clone()) else {
        return;
    };

    let Some(tx) = &network_state.message_sender else {
        return;
    };

    // Create spectator-safe message (no sensitive player data)
    let spectator_msg = NetworkMessage::GameStateBroadcast {
        game_id: game_id.parse().unwrap_or(0),
        fen: game_state.current_fen.clone(),
        last_move: game_state.last_move.clone(),
        move_number: game_state.move_number,
        is_check: game_state.is_check,
    };

    // Compress the message
    if let Ok(compressed) = compress_message(&spectator_msg) {
        let topic = format!("/xfchess-game/{}", game_id);
        let _ = tx.send((topic, compressed));
    }
}

/// System to handle spectator subscriptions
pub fn handle_spectator_subscriptions(
    mut network_events: EventReader<NetworkEvent>,
    mut spectator_registry: ResMut<SpectatorRegistry>,
) {
    for event in network_events.read() {
        match event {
            NetworkEvent::PeerDiscovered(info) if info.role == NodeRole::Spectator => {
                if let Some(game_id) = info.connected_game {
                    let topic = format!("/xfchess-game/{}", game_id);
                    spectator_registry.add_spectator(&topic, info.node_id.clone());
                }
            }
            NetworkEvent::PeerDisconnected { node_id } => {
                spectator_registry.remove_spectator(node_id);
            }
            _ => {}
        }
    }
}

/// Compress a NetworkMessage for gossip broadcast
fn compress_message(msg: &NetworkMessage) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let serialized = bincode::serialize(msg)?;
    let compressed = compress(&serialized, COMPRESSION_LEVEL)?;
    Ok(compressed)
}

/// Decompress a received message
pub fn decompress_message(data: &[u8]) -> Result<NetworkMessage, Box<dyn std::error::Error>> {
    let decompressed = zstd::bulk::decompress(data, 1024 * 1024)?;
    let msg = bincode::deserialize(&decompressed)?;
    Ok(msg)
}

/// Plugin for spectator functionality
pub struct SpectatorPlugin;

impl Plugin for SpectatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorRegistry>()
           .add_systems(Update, (
               broadcast_game_state,
               handle_spectator_subscriptions,
           ));
    }
}
