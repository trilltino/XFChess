//! Tournament multiplayer module.
//!
//! Provides real-time tournament updates via braid-iroh gossip protocol.

pub mod client;
pub mod events;

pub use client::TournamentClientPlugin;
pub use events::TournamentEventsPlugin;

use bevy::prelude::*;

/// Combined plugin for tournament multiplayer
pub struct TournamentMultiplayerPlugin;

impl Plugin for TournamentMultiplayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((TournamentClientPlugin, TournamentEventsPlugin));
    }
}
