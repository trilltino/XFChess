// Ephemeral MVP Plugin for MagicBlock ER
#![allow(dead_code)]
//! MVP Plugin for MagicBlock integration
//!
//! Simple plugin to manage MagicBlock state and basic delegation lifecycle.
use bevy::prelude::*;

/// Ephemeral MVP state
#[derive(Resource, Default)]
pub struct EphemeralMvpState {
    pub is_initialized: bool,
    pub game_finalized: bool,
}

impl EphemeralMvpState {
    pub fn start_game(&mut self, _game_id: u64, _initial_fen: String) {
        self.is_initialized = true;
    }
}

pub struct EphemeralMvpPlugin;

impl Plugin for EphemeralMvpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EphemeralMvpState>();
    }
}
