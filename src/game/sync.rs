use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub mod board_state;

pub struct GameSyncPlugin;

impl Plugin for GameSyncPlugin {
    fn build(&self, _app: &mut App) {}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkMove {
    pub from: String,
    pub to: String,
    pub player: String,
    pub piece_type: String,
    pub captured_piece: Option<String>,
    pub promotion: Option<String>,
    pub timestamp: u64,
}
