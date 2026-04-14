#![allow(dead_code)]
//! Conditional rendering systems based on view mode
//!
//! Provides systems that run conditionally based on the current ViewMode.
//! This allows seamless switching between 2D and 3D rendering while
//! maintaining game state compatibility.

use crate::game::view_mode::{ViewMode, PlayerViewPreferences};
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

/// Run condition for TempleOS rendering systems
pub fn view_mode_is_templeos(view_preferences: Res<PlayerViewPreferences>) -> bool {
    matches!(view_preferences.local_view, ViewMode::TempleOS)
}

/// System to hide/show 3D entities based on view mode
pub fn toggle_3d_visibility(
    view_preferences: Res<PlayerViewPreferences>,
    mut piece_query: Query<&mut Visibility, With<Piece>>,
) {
    let show_3d = matches!(view_preferences.local_view, ViewMode::Standard3D | ViewMode::TempleOS);
    
    for mut visibility in &mut piece_query {
        *visibility = if show_3d {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
