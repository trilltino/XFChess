//! Camera module
//!
//! Manages camera setup and configuration.

pub mod camera_director;
/// TempleOS orthographic isometric camera — dev builds only.
#[cfg(feature = "templeos")]
pub mod camera_templeos;

use bevy::prelude::*;

// Re-export all public items
#[cfg(feature = "templeos")]
pub use camera_templeos::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        use crate::game::systems::camera::view_mode_toggle_input_system;
        app.add_systems(Update, view_mode_toggle_input_system);
    }
}
