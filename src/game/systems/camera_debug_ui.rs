//! Camera position debug UI
//!
//! Displays the camera's current XYZ position on screen for debugging
//! and development purposes.

use bevy::prelude::*;

use crate::core::{DespawnOnExit, GameState};
use crate::game::systems::CameraController;

/// Marker component for camera position display text
#[derive(Component)]
pub struct CameraPositionText;

/// System to spawn the camera position UI when entering InGame state
pub fn spawn_camera_position_ui(mut commands: Commands) {
    // Root node for positioning
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(10.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            DespawnOnExit(GameState::InGame),
            Name::new("Camera Position UI"),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Camera: X: 0.00, Y: 0.00, Z: 0.00"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                CameraPositionText,
            ));
        });
}

/// System to update camera position display each frame
pub fn update_camera_position_ui(
    camera_query: Query<&Transform, With<CameraController>>,
    mut text_query: Query<&mut Text, With<CameraPositionText>>,
) {
    // Get the camera with CameraController
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    // Update the text display
    for mut text in text_query.iter_mut() {
        let pos = camera_transform.translation;
        **text = format!("Camera: X: {:.2}, Y: {:.2}, Z: {:.2}", pos.x, pos.y, pos.z);
    }
}
