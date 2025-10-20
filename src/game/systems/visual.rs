//! Visual update systems for highlighting and animation

use bevy::prelude::*;
use crate::rendering::pieces::Piece;
use crate::rendering::utils::{Square, SquareMaterials, ReturnMaterials};
use crate::game::resources::Selection;

/// System to visually highlight possible moves and selected square
///
/// This system runs every frame and updates square materials based on:
/// - Whether a piece is selected (highlights source square)
/// - Which squares are valid move destinations (highlights possible moves)
/// - Restores original colors for squares that are no longer highlighted
pub fn highlight_possible_moves(
    selection: Res<Selection>,
    square_materials: Res<SquareMaterials>,
    return_materials: Res<ReturnMaterials>,
    mut squares_query: Query<(Entity, &Square, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (_, square, mut material) in squares_query.iter_mut() {
        let pos = (square.x, square.y);

        // Check if this square should be highlighted
        let should_highlight = selection.is_selected() && (
            selection.selected_position == Some(pos) || // Selected square
            selection.possible_moves.contains(&pos)      // Valid move destination
        );

        if should_highlight {
            // Handle is Clone (not Copy), need .clone() from Res
            material.0 = square_materials.hover_matl.clone();
        } else {
            // Restore original color
            material.0 = return_materials.get_original_material(square, &square_materials);
        }
    }
}

/// System to animate piece movement
pub fn animate_piece_movement(
    time: Res<Time>,
    mut pieces_query: Query<(&mut Transform, &Piece)>,
) {
    for (mut transform, piece) in pieces_query.iter_mut() {
        let target = Vec3::new(piece.x as f32, 0., piece.y as f32);
        let current = transform.translation;

        // Smoothly move towards target position
        let direction = target - current;
        if direction.length() > 0.1 {
            let speed = 10.0; // units per second
            transform.translation += direction.normalize() * speed * time.delta_secs();
        } else {
            // Snap to final position when close enough
            transform.translation = target;
        }
    }
}
