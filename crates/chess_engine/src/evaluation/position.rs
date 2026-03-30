//! Full position evaluation
//!
//! Evaluates positions using material, piece-square tables, and mobility.

use super::pst::get_pst_value;
use crate::constants::*;
use crate::move_gen::*;
use crate::types::*;

/// Evaluate full position (material + positional)
pub fn evaluate_position(game: &Game) -> i16 {
    let mut score = 0i16;

    // Material and piece-square tables
    for square in 0..64 {
        let piece = game.board[square];
        if piece != 0 {
            // Material
            let piece_value = FIGURE_VALUE[piece.abs() as usize];
            score += if piece > 0 { piece_value } else { -piece_value };

            // Positional
            let pst_value = get_pst_value(piece, square as i8);
            score += pst_value;
        }
    }

    // Mobility bonus (simplified)
    let white_moves = count_moves(game, COLOR_WHITE);
    let black_moves = count_moves(game, COLOR_BLACK);
    score += (white_moves as i16 - black_moves as i16) * 5;

    score
}

/// Count number of pseudo-legal moves for a color
fn count_moves(game: &Game, color: Color) -> usize {
    generate_pseudo_legal_moves(game, color).len()
}
