
pub mod lobby;

pub use lobby::TournamentLobbyPlugin;

use bevy::prelude::*;

pub struct TournamentUiPlugin;

impl Plugin for TournamentUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TournamentLobbyPlugin);
    }
}
