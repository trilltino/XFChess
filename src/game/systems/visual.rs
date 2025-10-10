//! Visual update systems for highlighting and animation

use bevy::prelude::*;
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use crate::game::resources::Selection;

/// System to visually highlight possible moves
pub fn highlight_possible_moves(
    selection: Res<Selection>,
    mut materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
    squares_query: Query<(Entity, &Square)>,
    _hover_materials: Res<crate::rendering::utils::SquareMaterials>,
) {
    if !selection.is_selected() {
        return;
    }

    // Highlight squares where the selected piece can move
    for (entity, square) in squares_query.iter() {
        let pos = (square.x, square.y);
        if selection.possible_moves.contains(&pos) {
            if let Ok(_material) = materials.get_mut(entity) {
                // TODO: Use a different material for possible moves
                // material.0 = hover_materials.hover_matl.clone();
            }
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
