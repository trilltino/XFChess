//! Move ordering for alpha-beta pruning
//!
//! Orders moves to maximize alpha-beta pruning efficiency by trying
//! the best moves first (captures, center control, etc.).

use crate::board::*;
use crate::constants::*;
use crate::types::*;

/// Order moves for better alpha-beta pruning
pub(crate) fn order_moves(game: &Game, moves: &mut [KK], depth: i32) {
    let d_idx = depth.max(0) as usize;
    let killers = if d_idx <= MAX_DEPTH {
        game.killer_moves[d_idx]
    } else {
        [None; 2]
    };

    for mv in moves.iter_mut() {
        let mut score = 0i32;

        // 1. MVV-LVA: Most Valuable Victim - Least Valuable Attacker (Captures)
        let captured = game.board[mv.dst as usize];
        if captured != 0 {
            let attacker_value = FIGURE_VALUE[game.board[mv.src as usize].abs() as usize] as i32;
            let victim_value = FIGURE_VALUE[captured.abs() as usize] as i32;
            score += 10000 + (victim_value * 10 - attacker_value);
        }

        // 2. Killer Moves: Successful moves from other branches at same depth
        for killer in killers.iter().flatten() {
            if killer.src == mv.src && killer.dst == mv.dst {
                score += 5000;
                break;
            }
        }

        // 3. History Heuristic: Bonus for moves that frequently cause cutoffs
        let history = game.history_table[mv.src as usize][mv.dst as usize] as i32;
        score += history / 128; // Scale down history bonus

        // 4. Center control bonus (lower priority than heuristics)
        let (col, row) = pos_to_square(mv.dst);
        let center_dist = ((col - 3).abs() + (row - 3).abs()) as i32;
        score += (8 - center_dist) * 2;

        mv.score = score.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }

    // Sort moves by score (descending)
    moves.sort_by(|a, b| b.score.cmp(&a.score));
}
