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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::new_game;
    use crate::constants::{B_QUEEN, PAWN_VALUE, QUEEN_VALUE, W_QUEEN};

    #[test]
    fn test_starting_position_material_balance() {
        let game = new_game();
        let score = evaluate_material(&game);
        assert_eq!(score, 0, "Starting position should have 0 material balance");
    }

    #[test]
    fn test_white_up_queen() {
        let mut game = new_game();
        // Remove black queen from d8 (position 59)
        game.board[60] = 0; // Using 60 based on engine's layout

        let score = evaluate_material(&game);
        assert!(score > 0, "White should be winning without black queen");
        assert!(score >= QUEEN_VALUE, "Score should be at least queen value");
    }

    #[test]
    fn test_black_up_pawn() {
        let mut game = new_game();
        // Remove white pawn from e2 (position 12)
        game.board[12] = 0;

        let score = evaluate_material(&game);
        assert!(score < 0, "Black should be winning without white pawn");
        assert_eq!(
            score, -PAWN_VALUE,
            "Score should be exactly negative pawn value"
        );
    }

    #[test]
    fn test_empty_board_material() {
        let mut game = new_game();
        for i in 0..64 {
            game.board[i] = 0;
        }

        let score = evaluate_material(&game);
        assert_eq!(score, 0, "Empty board should have 0 material");
    }
}
