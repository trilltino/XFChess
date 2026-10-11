//! 3D presentation: board, pieces, cameras, lighting and move feedback. It reads
//! game state and never changes it. See [README.md](README.md).

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
        ));
        graphics_quality::register(app);
    }
}
