//! Coordinate label system for chess board
//!
//! Spawns floating coordinate labels (numbers 1-8 and letters A-H) around the chess board
//! for the TempleOS view mode. Labels are positioned at the edges of the board and
//! float above it for visibility.

use crate::core::GameState;
use crate::game::view_mode::ViewMode;
use bevy::prelude::*;

/// Marker component for coordinate labels
#[derive(Component)]
pub struct CoordinateLabel;

/// System to create coordinate labels for TempleOS view
///
/// Spawns:
/// - Numbers 1-8 along the left and right edges (vertical axis)
/// - Letters A-H along the front and back edges (horizontal axis)
///
/// Uses Text2d positioned in 3D space with black color to match reference image.
pub fn create_coordinate_labels(
    mut commands: Commands,
    view_mode: Res<ViewMode>,
    _asset_server: Res<AssetServer>,
) {
    // Only create labels in TempleOS mode
    if *view_mode != ViewMode::TempleOS {
        return;
    }

    // Black text style for labels (visible on yellow background)
    // Use default font (works if default_font feature is enabled, otherwise will need a font file)
    let text_style = TextFont {
        font_size: 24.0,
        ..default() // Uses default font
    };

    // Spawn number labels (1-8) along the left edge (X = -0.5)
    for rank in 1..=8 {
        let z_pos = (rank - 1) as f32 + 0.5;

        commands.spawn((
            Text2d::new(rank.to_string()),
            text_style.clone(),
            TextColor(Color::srgb(0.0, 0.0, 0.0)), // Black text
            TextLayout::default(),
            Transform::from_translation(Vec3::new(-0.5, 0.1, z_pos)),
            CoordinateLabel,
            DespawnOnExit(GameState::InGame),
            Name::new(format!("Label Rank {}", rank)),
        ));
    }

    // Spawn number labels (1-8) along the right edge (X = 7.5)
    for rank in 1..=8 {
        let z_pos = (rank - 1) as f32 + 0.5;

        commands.spawn((
            Text2d::new(rank.to_string()),
            text_style.clone(),
            TextColor(Color::srgb(0.0, 0.0, 0.0)), // Black text
            TextLayout::default(),
            Transform::from_translation(Vec3::new(7.5, 0.1, z_pos)),
            CoordinateLabel,
            DespawnOnExit(GameState::InGame),
            Name::new(format!("Label Rank {} Right", rank)),
        ));
    }

    // Spawn letter labels (A-H) along the front edge (Z = -0.5)
    for (file_idx, file_char) in ('a'..='h').enumerate() {
        let x_pos = file_idx as f32 + 0.5;
        let label = file_char.to_uppercase().to_string();

        commands.spawn((
            Text2d::new(label),
            text_style.clone(),
            TextColor(Color::srgb(0.0, 0.0, 0.0)), // Black text
            TextLayout::default(),
            Transform::from_translation(Vec3::new(x_pos, 0.1, -0.5)),
            CoordinateLabel,
            DespawnOnExit(GameState::InGame),
            Name::new(format!("Label File {}", file_char)),
        ));
    }

    // Spawn letter labels (A-H) along the back edge (Z = 7.5)
    for (file_idx, file_char) in ('a'..='h').enumerate() {
        let x_pos = file_idx as f32 + 0.5;
        let label = file_char.to_uppercase().to_string();

        commands.spawn((
            Text2d::new(label),
            text_style.clone(),
            TextColor(Color::srgb(0.0, 0.0, 0.0)), // Black text
            TextLayout::default(),
            Transform::from_translation(Vec3::new(x_pos, 0.1, 7.5)),
            CoordinateLabel,
            DespawnOnExit(GameState::InGame),
            Name::new(format!("Label File {} Back", file_char)),
        ));
    }

    info!("[COORDINATES] Created {} black coordinate labels for TempleOS view (32 total: 16 numbers + 16 letters)", 32);
}
