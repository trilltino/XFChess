//! Static Exchange Evaluation (SEE) — simplified MVV-LVA approximation
//!
//! A full SEE implementation would use least-valuable-attacker recursion.
//! This version uses a practical MVV-LVA approximation that is sufficient
//! for move ordering and pruning decisions.

use crate::constants::*;
use crate::types::*;

/// Evaluate whether a capture is likely winning material.
/// Returns `true` if the exchange value is >= threshold.
///
/// For captures: value = victim_value - attacker_value (+ promotion bonus)
/// For non-captures: always true.
pub fn see(game: &Game, mv: KK, threshold: i32) -> bool {
    let src = mv.src as usize;
    let dst = mv.dst as usize;

    let victim = game.board[dst];
    let attacker_piece = game.board[src];

    // Not a capture and not a promotion — SEE doesn't apply
    if victim == 0 && (mv.nxt_dir_idx >> 4) == 0 {
        return true;
    }

    let attacker_type = attacker_piece.abs() as usize;
    let victim_type = if victim != 0 { victim.abs() as usize } else { 0 };

    // Base exchange value
    let mut value = if victim != 0 {
        FIGURE_VALUE[victim_type] as i32
    } else {
        0
    };

    // Subtract attacker value (it will be recaptured)
    value -= FIGURE_VALUE[attacker_type] as i32;

    // Promotion bonus
    let promo = (mv.nxt_dir_idx >> 4) as i8;
    if promo != 0 && attacker_type == PAWN_ID as usize {
        value += FIGURE_VALUE[promo.abs() as usize] as i32;
    }

    value >= threshold
}
