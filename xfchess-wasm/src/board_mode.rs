//! Board render mode toggle (3D ↔ 2D).

use bevy::prelude::*;

/// Current board rendering mode.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardRenderMode {
    ThreeD,
    TwoD,
}

/// Toggle between 3D and 2D when the user presses Tab.
pub fn toggle_board_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<BoardRenderMode>,
) {
    if keyboard.just_pressed(KeyCode::Tab) {
        *mode = match *mode {
            BoardRenderMode::ThreeD => BoardRenderMode::TwoD,
            BoardRenderMode::TwoD => BoardRenderMode::ThreeD,
        };
        tracing::info!("[wasm] Board mode: {:?}", *mode);
    }
}
