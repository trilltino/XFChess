use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Represents the synchronized state of a chess game over Braid.
/// This matches the legacy structure but we now prefer ChessMessage.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Reflect)]
#[reflect(Default)]
pub struct BraidGameState {
    pub fen: String,
    pub last_move: Option<String>,
    pub is_white_turn: bool,
    pub status: GameStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Reflect)]
pub enum GameStatus {
    #[default]
    Playing,
    Checkmate,
    Stalemate,
    Resigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum BraidConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Subscribed,
    Error,
}

#[derive(Resource, Default, Clone, Reflect)]
#[reflect(Resource)]
pub struct BraidP2PConfig {
    pub base_url: String,
    pub game_id: String,
    pub active: bool,
}

/// Message for notifying Bevy about network incoming states
#[derive(Message, Debug)]
pub struct NetworkGameStateUpdated(pub BraidGameState);

