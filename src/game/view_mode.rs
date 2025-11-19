//! View mode resource for chess board visualization
//!
//! Controls different visual styles for the chess board, including
//! the TempleOS-inspired view with grey/white board and coordinate labels.

use bevy::prelude::*;

/// View mode for chess board visualization
///
/// Determines the visual style and camera setup for the chess game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Resource, Default)]
#[reflect(Resource)]
pub enum ViewMode {
    /// Standard view with black/white board and RTS-style camera
    #[default]
    Standard,

    /// TempleOS-inspired view with grey/white board, diagonal camera, and coordinate labels
    TempleOS,
}

impl ViewMode {
    /// Returns a human-readable description of the view mode
    pub fn description(self) -> &'static str {
        match self {
            ViewMode::Standard => "Standard View",
            ViewMode::TempleOS => "TempleOS View",
        }
    }
}
