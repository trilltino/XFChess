//! An early, minimal MagicBlock state tracker. Confirmed unused — neither
//! `EphemeralMvpState` nor `EphemeralMvpPlugin` is referenced anywhere else
//! in the codebase; the real ER delegation lifecycle lives in
//! `rollup::magicblock` and `rollup::manager` instead.

use bevy::prelude::*;

/// Unused. See module docs.
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

/// Unused. See module docs.
pub struct EphemeralMvpPlugin;

impl Plugin for EphemeralMvpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EphemeralMvpState>();
    }
}
