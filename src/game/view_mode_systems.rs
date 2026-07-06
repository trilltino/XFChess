//! Conditional rendering systems based on view mode
//!
//! Provides systems that run conditionally based on the current ViewMode.
//! This allows seamless switching between 2D and 3D rendering while
//! maintaining game state compatibility.

use crate::game::view_mode::{PlayerViewPreferences, ViewMode};
use crate::rendering::pieces::Piece;
use bevy::prelude::*;

/// Run condition for 3D rendering systems
pub fn view_mode_is_3d(view_preferences: Res<PlayerViewPreferences>) -> bool {
    matches!(view_preferences.local_view, ViewMode::Standard3D)
}

/// Run condition for 2D rendering systems
pub fn view_mode_is_2d(view_preferences: Res<PlayerViewPreferences>) -> bool {
    matches!(view_preferences.local_view, ViewMode::Standard2D)
}

/// System to hide/show 3D entities based on view mode
pub fn toggle_3d_visibility(
    _view_preferences: Res<PlayerViewPreferences>,
    _piece_query: Query<&mut Visibility, With<Piece>>,
) {
    // This system is now redundant as piece-specific visuals are handled
    // in rendering::pieces::view_mode_rendering_toggle_system
}
