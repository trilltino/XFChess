//! Input module - picking and observer patterns.

pub mod pointer;
pub use pointer::*;

use bevy::prelude::*;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, _app: &mut App) {
        // PointerInputPlugin is already included in Bevy's DefaultPlugins
        // Custom input systems would be added here
    }
}
