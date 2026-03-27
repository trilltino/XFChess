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
