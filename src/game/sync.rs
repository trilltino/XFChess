use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// Board state sync module
pub mod board_state;

/// Plugin for game state synchronization
pub struct GameSyncPlugin;

impl Plugin for GameSyncPlugin {
    fn build(&self, _app: &mut App) {
        // Game state synchronization systems are disabled for singleplayer mode
        // Network sync will be re-enabled when multiplayer is refactored
    }
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
