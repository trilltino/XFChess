//! Singleplayer component entry point
pub mod local_input;

use bevy::prelude::*;

pub struct SingleplayerPlugin;

impl Plugin for SingleplayerPlugin {
    fn build(&self, _app: &mut App) {
        // Additional singleplayer specific logic
    }
}
