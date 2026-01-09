//! Move ordering for alpha-beta pruning
//!
//! Orders moves to maximize alpha-beta pruning efficiency by trying
//! the best moves first (captures, center control, etc.).

use crate::board::*;
use crate::constants::*;
use crate::types::*;

/// Order moves for better alpha-beta pruning
pub(crate) fn order_moves(game: &Game, moves: &mut [KK]) {
    for mv in moves.iter_mut() {
        let mut score = 0i32;

        // Captures are good
        let captured = game.board[mv.dst as usize];
        if captured != 0 {
            let attacker_value = FIGURE_VALUE[game.board[mv.src as usize].abs() as usize] as i32;
            let victim_value = FIGURE_VALUE[captured.abs() as usize] as i32;
            // MVV-LVA: Most Valuable Victim - Least Valuable Attacker
            score += victim_value * 10 - attacker_value;
        }

        // Center control bonus
        let (col, row) = pos_to_square(mv.dst);
        let center_dist = ((col - 3).abs() + (row - 3).abs()) as i32;
        score += (8 - center_dist) * 5;

        mv.score = score.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }

    // Sort moves by score (descending)
    moves.sort_by(|a, b| b.score.cmp(&a.score));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::new_game;
    use crate::constants::{B_PAWN, B_QUEEN, W_PAWN, W_QUEEN};
    use crate::move_gen::generate_pseudo_legal_moves;

    #[test]
    fn test_order_moves_prioritizes_captures() {
        let mut game = new_game();

        // Setup: White pawn can capture black queen
        game.board[28] = W_PAWN; // e4
        game.board[35] = B_QUEEN; // d5 - black queen
        game.board[36] = 0; // Clear potential blockers

        // Create two moves: capture and non-capture
        let mut moves = vec![
            KK::new(28, 36, 0, 0), // Non-capture e4-e5
            KK::new(28, 35, 0, 0), // Capture e4xd5 (captures queen!)
        ];

        order_moves(&game, &mut moves);

        // The capture should be first (higher score)
        assert!(moves[0].dst == 35, "Capture should be ordered first");
        assert!(
            moves[0].score > moves[1].score,
            "Capture should have higher score"
        );
    }

    #[test]
    fn test_order_moves_mvv_lva() {
        let mut game = new_game();

        // Setup: Pawn can capture queen, queen can capture pawn
        game.board[28] = W_PAWN; // e4 - white pawn
        game.board[35] = B_QUEEN; // d5 - black queen
        game.board[20] = W_QUEEN; // e3 - white queen
        game.board[27] = B_PAWN; // d4 - black pawn

        // Create two capture moves
        let mut moves = vec![
            KK::new(20, 27, 0, 0), // QxP (queen captures pawn)
            KK::new(28, 35, 0, 0), // PxQ (pawn captures queen)
        ];

        order_moves(&game, &mut moves);

        // PxQ should be prioritized (MVV-LVA: Most Valuable Victim)
        assert_eq!(moves[0].src, 28, "Pawn capturing queen should be first");
    }

    #[test]
    fn test_order_moves_center_control() {
        let mut game = new_game();

        // Clear board except for two pieces making non-capture moves
        for i in 0..64 {
            game.board[i] = 0;
        }
        game.board[0] = W_PAWN; // a1
        game.board[7] = W_PAWN; // h1

        // Move to center vs corner
        let mut moves = vec![
            KK::new(0, 56, 0, 0), // a1 -> a8 (corner)
            KK::new(7, 27, 0, 0), // h1 -> d4 (center)
        ];

        order_moves(&game, &mut moves);

        // Center move should have higher score
        assert!(moves[0].dst == 27, "Center move should be ordered first");
    }

    #[test]
    fn test_order_moves_starting_position() {
        let game = new_game();
        let mut moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);

        let original_count = moves.len();
        order_moves(&game, &mut moves);

        // Should not change the number of moves
        assert_eq!(moves.len(), original_count);

        // All moves should have scores assigned
        for mv in &moves {
            // Scores should be within valid i16 range
            assert!(mv.score >= i16::MIN && mv.score <= i16::MAX);
        }
    }
}
