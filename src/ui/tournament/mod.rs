//! Tournament UI module.
//!
//! Provides UI components for Swiss tournament lobbies and brackets.

pub mod lobby;

pub use lobby::TournamentLobbyPlugin;

use bevy::prelude::*;

/// Combined plugin for all tournament UI
pub struct TournamentUiPlugin;

impl Plugin for TournamentUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TournamentLobbyPlugin);
    }
}
