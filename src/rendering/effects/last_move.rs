//! Last move highlighting system
//!
//! Highlights the from/to squares of the last move when highlight_last_move is enabled.

use crate::core::GameSettings;
use crate::game::resources::MoveHistory;
use crate::rendering::utils::SquareMaterials;
use bevy::prelude::*;

/// Marker component for squares showing last move highlight
#[derive(Component)]
pub struct LastMoveHighlight;

/// System that shows/hides last move highlights based on settings
///
/// When highlight_last_move is enabled, highlights the from and to squares of the last move.
/// System that shows/hides last move highlights based on settings
///
/// Optimized to only update when move history or settings change, and reuses shared meshes.
pub fn update_last_move_highlight_system(
    mut commands: Commands,
    settings: Res<GameSettings>,
    move_history: Res<MoveHistory>,
    highlight_query: Query<Entity, With<LastMoveHighlight>>,
    materials: Res<SquareMaterials>,
) {
    // Only update if move history or settings changed
    if !move_history.is_changed() && !settings.is_changed() {
        return;
    }

    // Always clear old highlights on change
    for entity in highlight_query.iter() {
        commands.entity(entity).despawn();
    }

    if settings.highlight_last_move {
        if let Some(last_move) = move_history.last_move() {
            let positions = [last_move.from, last_move.to];

            for (x, y) in positions {
                commands.spawn((
                    Mesh3d(materials.highlight_mesh.clone()),
                    MeshMaterial3d(materials.hover_matl.clone()),
                    Transform::from_translation(Vec3::new(x as f32, 0.02, y as f32)),
                    LastMoveHighlight,
                    Name::new("Last Move Highlight"),
                    // Ensure highlights are cleaned up if state exits
                    crate::core::DespawnOnExit(crate::core::GameState::InGame),
                ));
            }
        }
    }
}
