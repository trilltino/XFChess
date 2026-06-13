//! Iterative deepening search
//!
//! Implements iterative deepening to enable time management and progressive
//! search depth increases.

use super::alphabeta::alphabeta;
use super::make_unmake::{make_move, unmake_move};
use super::params::SearchParams;
use crate::constants::*;
use crate::hash::*;
use crate::move_gen::is_in_check;
use crate::move_gen::*;
use crate::types::*;
use std::time::Instant;
use core::sync::atomic::Ordering;

static SP: SearchParams = SearchParams::sarah_tuned();

/// Iterative deepening search (Synchronous)
pub fn iterative_deepening(game: &mut Game, max_time_secs: f32, color: Color) -> Move {
    let start_time = Instant::now();
    let mut best_move = Move::default();
    let mut best_score = LOWEST_SCORE as i16;

    // Reset statistics
    game.calls = 0;
    game.cut = 0;
    game.tte_hit = 0;
    game.abort_search.store(false, Ordering::Relaxed);

    // Time budget: a hard wall-clock deadline polled inside the search (so a
    // long iteration aborts instead of flagging), plus a soft limit below
    // that stops starting new iterations (the next iteration typically costs
    // 2-4x the previous one).
    let budget = max_time_secs.max(0.01);
    let soft_limit = budget * 0.7;
    game.search_deadline = Some(start_time + std::time::Duration::from_secs_f32(budget * 0.95));

    let mut prev_score = 0i16;

    // Respect a caller-set nominal depth limit (UCI `go depth N`); 0 or
    // negative means "no limit beyond MAX_DEPTH".
    let depth_limit = if game.abs_max_depth > 0 {
        (game.abs_max_depth as usize).min(MAX_DEPTH)
    } else {
        MAX_DEPTH
    };
    for depth in 1..=depth_limit {
        // Aspiration windows after depth 1
        let (alpha, beta) = if depth > 1 {
            let window = (SP.aspiration_base as i16 + (SP.aspiration_mul * depth as i32) as i16)
                .max(10);
            (prev_score.saturating_sub(window), prev_score.saturating_add(window))
        } else {
            (-AB_INF, AB_INF)
        };

        // Handle search errors gracefully
        let mut score = match alphabeta(game, depth as i32, alpha, beta, color) {
            Ok(score) => score,
            Err(e) => {
                // Log the error but continue with fallback
                eprintln!("[CHESS_ENGINE] Search error at depth {}: {}", depth, e);
                // If we have a previous best_score, use it; otherwise break
                if best_score != LOWEST_SCORE as i16 {
                    break;
                } else {
                    // No valid score yet - try to find any legal move as fallback
                    break;
                }
            }
        };

        // Re-search on fail-low or fail-high
        if score <= alpha || score >= beta {
            score = match alphabeta(game, depth as i32, -AB_INF, AB_INF, color) {
                Ok(s) => s,
                Err(_) => break,
            };
        }

        prev_score = score;

        // If the iteration was aborted (deadline hit inside the search or an
        // external stop), its score is untrustworthy — keep the previous
        // iteration's move and stop.
        if game.abort_search.load(Ordering::Relaxed) {
            break;
        }

        game.max_depth_so_far = depth as i64;
        best_score = score;
        update_best_move_from_tt(game, &mut best_move, score);
        #[cfg(feature = "salewskiChessDebug")]
        eprintln!(
            "[id] depth {} score {} tt_best {}->{}",
            depth, score, best_move.src, best_move.dst
        );

        // Check for checkmate
        if score.abs() > KING_VALUE_DIV_2 {
            best_move.state = STATE_CHECKMATE;
            best_move.checkmate_in = ((KING_VALUE - score.abs()) / 2) as i64;
            break;
        }

        // Soft limit: don't start an iteration we can't plausibly finish.
        if start_time.elapsed().as_secs_f32() > soft_limit {
            break;
        }
    }

    game.search_deadline = None;

    // If no move found, find any legal move
    if best_move.src == 0 && best_move.dst == 0 {
        let moves = generate_pseudo_legal_moves(game, color);
        for mv in moves {
            let undo = make_move(game, mv);
            let legal = !is_in_check(game, color);
            unmake_move(game, mv, undo);

            if legal {
                best_move.src = mv.src as i64;
                best_move.dst = mv.dst as i64;
                best_move.score = best_score as i64;
                best_move.promo = (mv.nxt_dir_idx >> 4) as i8;
                break;
            }
        }
    }

    best_move
}

fn update_best_move_from_tt(game: &Game, best_move: &mut Move, score: i16) {
    let hash = position_hash(game);
    if let Some(cached) = tt_probe(game, &hash) {
        if !cached.h.is_empty() {
            let guide = &cached.h[0];
            best_move.src = guide.best_move_src as i64;
            best_move.dst = guide.best_move_dst as i64;
            best_move.score = score as i64;
            best_move.promo = (guide.best_move_nxt_dir_idx >> 4) as i8;
        }
    }
}

/// Find best move for current position
pub fn find_best_move(game: &mut Game, think_time: f32, color: Color) -> Move {
    iterative_deepening(game, think_time, color)
}
