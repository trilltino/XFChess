//! Full position evaluation
//!
//! Evaluates positions using tapered material + piece-square tables (MG/EG),
//! mobility, and attack bonuses.

use super::pst::get_pst_value_tapered;
use crate::constants::*;
use crate::move_gen::*;
use crate::types::*;

/// Evaluate full position (tapered material + positional + mobility)
pub fn evaluate_position(game: &Game) -> i16 {
    let mut score = 0i16;
    let mut phase = 0i16;

    // Material and piece-square tables (tapered MG/EG)
    for square in 0..64 {
        let piece = game.board[square];
        if piece != 0 {
            let abs_piece = piece.abs() as usize;
            // Material
            let piece_value = FIGURE_VALUE[abs_piece];
            score += if piece > 0 { piece_value } else { -piece_value };

            // Phase accumulation
            phase += PHASE_VALUES[abs_piece];

            // Positional (tapered)
            let pst_value = get_pst_value_tapered(piece, square as i8, phase.min(MAX_PHASE));
            score += pst_value;
        }
    }

    // Clamp phase to valid range
    let phase = phase.min(MAX_PHASE);

    // Mobility bonus (tapered: more important in endgame)
    let white_moves = count_moves(game, COLOR_WHITE) as i16;
    let black_moves = count_moves(game, COLOR_BLACK) as i16;
    let mobility = white_moves - black_moves;
    // Mobility is more important in endgame
    let mobility_bonus = (mobility * 3 * (MAX_PHASE - phase) + mobility * 2 * phase) / MAX_PHASE;
    score += mobility_bonus;

    score
}

/// Count number of pseudo-legal moves for a color
fn count_moves(game: &Game, color: Color) -> usize {
    generate_pseudo_legal_moves(game, color).len()
}
