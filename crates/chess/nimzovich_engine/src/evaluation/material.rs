//! Material evaluation
//!
//! Evaluates the material balance of a position by counting piece values.

use crate::constants::*;
use crate::types::*;

/// Evaluate material balance
#[allow(dead_code)] // Part of engine's public API - evaluation function
pub fn evaluate_material(game: &Game) -> i16 {
    let mut score = 0i16;

    for square in 0..64 {
        let piece = game.board[square];
        if piece != 0 {
            let piece_value = FIGURE_VALUE[piece.abs() as usize];
            score += if piece > 0 { piece_value } else { -piece_value };
        }
    }

    score
}
