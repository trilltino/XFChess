//! Iterative deepening search
//!
//! Implements iterative deepening to enable time management and progressive
//! search depth increases.

use super::alphabeta::alphabeta;
use super::make_unmake::{make_move, unmake_move};
use crate::constants::*;
use crate::hash::*;
use crate::move_gen::is_in_check;
use crate::move_gen::*;
use crate::types::*;
use instant::Instant;

/// Iterative deepening search
pub async fn iterative_deepening(game: &mut Game, max_time_secs: f32, color: Color) -> Move {
    let start_time = Instant::now();
    let mut best_move = Move::default();
    let mut best_score = LOWEST_SCORE as i16;

    // Reset statistics
    game.calls = 0;
    game.cut = 0;
    game.tte_hit = 0;

    for depth in 1..=MAX_DEPTH {
        // Handle search errors gracefully
        let score = match alphabeta(game, depth as i32, -AB_INF, AB_INF, color).await {
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

        // Check time
        if start_time.elapsed().as_secs_f32() > max_time_secs * 0.9 {
            break;
        }

        game.max_depth_so_far = depth as i64;
        best_score = score;

        // Get best move from TT
        let hash = position_hash(&game.board);
        if let Some(cached) = tt_probe(game, &hash) {
            if !cached.h.is_empty() {
                let guide = &cached.h[0];
                best_move.src = guide.best_move_src as i64;
                best_move.dst = guide.best_move_dst as i64;
                best_move.score = score as i64;
            }
        }

        // Check for checkmate
        if score.abs() > KING_VALUE_DIV_2 {
            best_move.state = STATE_CHECKMATE;
            best_move.checkmate_in = ((KING_VALUE - score.abs()) / 2) as i64;
            break;
        }
    }

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
                break;
            }
        }
    }

    best_move
}

/// Find best move for current position
pub async fn find_best_move(game: &mut Game, think_time: f32, color: Color) -> Move {
    iterative_deepening(game, think_time, color).await
}
