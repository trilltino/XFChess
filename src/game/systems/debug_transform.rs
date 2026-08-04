use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::rendering::utils::Square;
use bevy::prelude::*;

pub fn debug_log_transforms(
    time: Res<Time>,
    mut timer: Local<f32>,
    pieces: Query<(&Piece, &GlobalTransform)>,
    squares: Query<(&Square, &GlobalTransform)>,
) {
    *timer += time.delta_secs();
    if *timer < 5.0 {
        return;
    } // Log every 5 seconds
    *timer = 0.0;

    // Log White King Position
    for (piece, transform) in pieces.iter() {
        if piece.piece_type == PieceType::King && piece.color == PieceColor::White {
            debug!(
                "[DEBUG_TRANSFORM] White King (Logical: {:?}) -> World: {:?}",
                (piece.x, piece.y),
                transform.translation()
            );
        }
        if piece.piece_type == PieceType::Pawn && piece.color == PieceColor::White && piece.y == 0 {
            debug!(
                "[DEBUG_TRANSFORM] White Pawn a2 (Logical: {:?}) -> World: {:?}",
                (piece.x, piece.y),
                transform.translation()
            );
        }
    }

    // Log Square a1 Position (0,0)
    for (square, transform) in squares.iter() {
        if square.x == 0 && square.y == 0 {
            debug!(
                "[DEBUG_TRANSFORM] Square a1 (Logical: 0,0) -> World: {:?}",
                transform.translation()
            );
        }
        if square.x == 1 && square.y == 0 {
            debug!(
                "[DEBUG_TRANSFORM] Square a2 (Logical: 1,0) -> World: {:?}",
                transform.translation()
            );
        }
    }
}
