pub mod client;
pub mod events;

pub use client::TournamentClientPlugin;
pub use events::TournamentEventsPlugin;

use bevy::prelude::*;

pub struct TournamentMultiplayerPlugin;

impl Plugin for TournamentMultiplayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((TournamentClientPlugin, TournamentEventsPlugin));
    }
}
