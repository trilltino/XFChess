//! Rendering module - 3D chess visualization.

pub mod board;
pub mod camera;
pub mod effects;
pub mod pieces;

// Root-level modules
pub mod graphics_quality;
pub mod utils;

// Re-export commonly used items
pub use camera::*;
pub use effects::*;
pub use pieces::*;

use bevy::prelude::*;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            utils::BoardUtils,
            board::BoardPlugin,
            camera::CameraPlugin,
            pieces::PiecePlugin,
            effects::DynamicLightingPlugin,
            effects::SkyPlugin,
        ))
        .add_systems(
            Update,
            (
                graphics_quality::apply_graphics_quality_camera_system,
                graphics_quality::update_graphics_quality_camera_system,
                graphics_quality::apply_graphics_quality_lights_system,
            ),
        );
    }
}
