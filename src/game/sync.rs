use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub mod board_state;

/// Registered in `GamePlugin` but currently a no-op — `build` adds no
/// systems. Actual multiplayer move sync runs through
/// `multiplayer::network`/`multiplayer::rollup`, not through this plugin.
pub struct GameSyncPlugin;

impl Plugin for GameSyncPlugin {
    fn build(&self, _app: &mut App) {}
}

/// Represents a move transmitted over the network. Not currently constructed
/// or read anywhere — `multiplayer::network`'s own move types are used
/// instead for real network sync.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkMove {
    /// Source square (e.g. "e2").
    pub from: String,
    /// Destination square (e.g. "e4").
    pub to: String,
    pub player: String,
    pub piece_type: String,
    /// Type of captured piece, if any.
    pub captured_piece: Option<String>,
    /// Promotion piece, if a pawn was promoted.
    pub promotion: Option<String>,
    pub timestamp: u64,
}
