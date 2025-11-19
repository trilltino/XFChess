//! Last move highlighting system
//!
//! Highlights the from/to squares of the last move when highlight_last_move is enabled.

use crate::core::GameSettings;
use crate::game::resources::MoveHistory;
use crate::rendering::utils::{Square, SquareMaterials};
use crate::rendering::Board;
use bevy::prelude::*;

/// Marker component for squares showing last move highlight
#[derive(Component)]
pub struct LastMoveHighlight;

/// System that shows/hides last move highlights based on settings
///
/// When highlight_last_move is enabled, highlights the from and to squares of the last move.
pub fn update_last_move_highlight_system(
    mut commands: Commands,
    settings: Res<GameSettings>,
    move_history: Res<MoveHistory>,
    square_query: Query<(Entity, &Square), With<Board>>,
    highlight_query: Query<Entity, (With<LastMoveHighlight>, Without<Board>)>,
    materials: Res<SquareMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // First, remove all existing highlights
    for entity in highlight_query.iter() {
        commands.entity(entity).despawn();
    }

    if settings.highlight_last_move {
        if let Some(last_move) = move_history.last_move() {
            let from_pos = last_move.from;
            let to_pos = last_move.to;

            // Add highlights to from and to squares
            for (entity, square) in square_query.iter() {
                let pos = (square.x, square.y);
                if pos == from_pos || pos == to_pos {
                    // Spawn a highlight above the square
                    commands.entity(entity).with_children(|parent| {
                        parent.spawn((
                            Mesh3d(meshes.add(Plane3d::default().mesh().size(0.95, 0.95))),
                            MeshMaterial3d(materials.hover_matl.clone()),
                            Transform::from_translation(Vec3::new(0.0, 0.02, 0.0)),
                            LastMoveHighlight,
                            Name::new("Last Move Highlight"),
                        ));
                    });
                }
            }
        }
    }
}
