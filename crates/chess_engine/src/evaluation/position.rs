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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::new_game;
    use crate::constants::{B_QUEEN, COLOR_BLACK, COLOR_WHITE, W_QUEEN};

    #[test]
    fn test_starting_position_roughly_equal() {
        let game = new_game();
        let score = evaluate_position(&game);

        // Starting position should be roughly equal (within a few pawns)
        assert!(
            score.abs() < 100,
            "Starting position should be roughly equal, got {}",
            score
        );
    }

    #[test]
    fn test_white_up_queen_is_winning() {
        let mut game = new_game();
        // Remove black queen
        game.board[60] = 0;

        let score = evaluate_position(&game);
        assert!(
            score > 800,
            "White should be significantly winning without black queen, got {}",
            score
        );
    }

    #[test]
    fn test_mobility_affects_evaluation() {
        let mut game = new_game();

        // Original score
        let original = evaluate_position(&game);

        // Block most of black's pieces (place pawns in front)
        game.board[48] = W_PAWN;
        game.board[49] = W_PAWN;
        game.board[50] = W_PAWN;
        game.board[51] = W_PAWN;
        game.board[52] = W_PAWN;
        game.board[53] = W_PAWN;
        game.board[54] = W_PAWN;
        game.board[55] = W_PAWN;

        let blocked = evaluate_position(&game);

        // White should be better when black is blocked
        assert!(
            blocked > original,
            "White should be better when black is blocked"
        );
    }
}
