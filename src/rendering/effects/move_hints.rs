//! Move hints visualization system
//!
//! Highlights valid move squares when a piece is selected and show_hints is enabled.

use crate::core::GameSettings;
use crate::game::resources::Selection;
use crate::rendering::utils::{Square, SquareMaterials};
use crate::rendering::Board;
use bevy::prelude::*;

/// Marker component for squares showing move hints
#[derive(Component)]
pub struct MoveHint;

/// System that shows/hides move hints based on selection and settings
///
/// When show_hints is enabled and a piece is selected, highlights all valid move squares.
pub fn update_move_hints_system(
    mut commands: Commands,
    settings: Res<GameSettings>,
    selection: Res<Selection>,
    square_query: Query<(Entity, &Square), With<Board>>,
    hint_query: Query<Entity, (With<MoveHint>, Without<Board>)>,
    materials: Res<SquareMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let should_show_hints = settings.show_hints && selection.is_selected();

    // First, remove all existing hints
    for entity in hint_query.iter() {
        commands.entity(entity).despawn();
    }

    if should_show_hints {
        // Get valid move positions
        let valid_positions: std::collections::HashSet<(u8, u8)> =
            selection.possible_moves.iter().copied().collect();

        // Add hints to squares that are valid moves
        for (entity, square) in square_query.iter() {
            let pos = (square.x, square.y);
            if valid_positions.contains(&pos) {
                // Spawn a semi-transparent highlight above the square
                commands.entity(entity).with_children(|parent| {
                    parent.spawn((
                        Mesh3d(meshes.add(Plane3d::default().mesh().size(0.9, 0.9))),
                        MeshMaterial3d(materials.hover_matl.clone()),
                        Transform::from_translation(Vec3::new(0.0, 0.01, 0.0)),
                        MoveHint,
                        Name::new("Move Hint"),
                    ));
                });
            }
        }
    }
}
