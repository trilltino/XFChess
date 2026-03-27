//! Camera module
//!
//! Manages camera setup and configuration.

pub mod camera_templeos;

use bevy::prelude::*;

// Re-export all public items
pub use camera_templeos::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, camera_templeos::templeos_camera_movement_system);
    }
}
