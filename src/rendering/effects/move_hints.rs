//! Move hints visualization system
//!
//! Highlights valid move squares when a piece is selected and show_hints is enabled.

use crate::core::GameSettings;
use crate::game::resources::Selection;
use crate::rendering::utils::SquareMaterials;
use bevy::prelude::*;

/// Marker component for squares showing move hints
#[derive(Component)]
pub struct MoveHint;

/// System that shows/hides move hints based on selection and settings
///
/// When show_hints is enabled and a piece is selected, highlights all valid move squares.
/// System that shows/hides move hints based on selection and settings
///
/// Optimized to only update when selection or settings change, and reuses shared meshes.
pub fn update_move_hints_system(
    mut commands: Commands,
    settings: Res<GameSettings>,
    selection: Res<Selection>,
    hint_query: Query<Entity, With<MoveHint>>,
    materials: Res<SquareMaterials>,
) {
    // Only update if selection or settings changed
    // Note: We check is_changed() for specific resources to avoid per-frame work
    if !selection.is_changed() && !settings.is_changed() {
        return;
    }

    // Always clear old hints on change
    for entity in hint_query.iter() {
        commands.entity(entity).despawn();
    }

    if settings.show_hints && selection.is_selected() {
        let valid_positions = &selection.possible_moves;

        // Spawn hints for all valid moves using the shared mesh
        for &(x, y) in valid_positions {
            commands.spawn((
                Mesh3d(materials.hint_mesh.clone()),
                MeshMaterial3d(materials.hover_matl.clone()),
                Transform::from_translation(Vec3::new(x as f32, 0.01, y as f32)),
                MoveHint,
                Name::new("Move Hint"),
                // Ensure hints are cleaned up if state exits
                crate::core::DespawnOnExit(crate::core::GameState::InGame),
            ));
        }
    }
}
