//! Alpha-beta search with negamax (RECURSIVE VERSION)
//!
//! This module implements the core search algorithm using a synchronous,
//! recursive approach which is more efficient for the compiler to optimize.

use super::make_unmake::{make_move, unmake_move};
use super::ordering::order_moves;
use super::quiescence::quiescence_search;
use crate::constants::*;
use crate::error::ChessEngineResult;
use crate::hash::*;
use crate::move_gen::*;
use crate::types::*;
use core::sync::atomic::Ordering;

/// Alpha-beta search with negamax (Synchronous Recursive)
///
/// # Arguments
///
/// * `game` - The current game state
/// * `depth` - Current search depth remaining
/// * `alpha` - Lower bound for the score
/// * `beta` - Upper bound for the score
/// * `color` - Side to move (1 for White, -1 for Black)
///
/// # Returns
///
/// The best score for the current player
pub(crate) fn alphabeta(
    game: &mut Game,
    mut depth: i32,
    mut alpha: i16,
    beta: i16,
    color: Color,
) -> ChessEngineResult<i16> {
    // Check for search abort
    if game.calls % 1024 == 0 && game.abort_search.load(Ordering::Relaxed) {
        return Ok(0); // Return a neutral score when aborted
    }

    game.calls += 1;

    // Base case: depth 0 - do quiescence search
    if depth <= 0 {
        return quiescence_search(game, alpha, beta, color);
    }

    // Transposition table probe
    let hash = position_hash(game);
    if let Some(cached) = tt_probe(game, &hash) {
        if cached.depth >= depth as i64 {
            game.tte_hit += 1;
            if !cached.h.is_empty() && cached.h[0].score != INVALID_SCORE {
                return Ok(cached.h[0].score);
            }
        }
    }

    // Check detection and depth extension
    let in_check = is_in_check(game, color);
    if in_check {
        depth += 1;
    }

    // Null Move Pruning (NMP)
    if depth >= 3 && !in_check && crate::board::has_non_pawn_material(game, color) {
        crate::hash::toggle_turn(game);
        // Search with reduced depth (R=2) and null window
        let nm_score = -alphabeta(game, depth - 3, -beta, -beta + 1, -color)?;
        crate::hash::toggle_turn(game);

        if nm_score >= beta {
            return Ok(beta);
        }
    }

    // Generate moves
    let mut moves = generate_pseudo_legal_moves(game, color);
    if moves.is_empty() {
        return Ok(if in_check {
            -KING_VALUE + (100 - depth) as i16 // Checkmate
        } else {
            0 // Stalemate
        });
    }

    // Order moves for better pruning
    order_moves(game, &mut moves, depth);

    let mut best_score = -AB_INF;
    let mut best_move = moves[0];
    let mut legal_move_count = 0;

    for mv in moves {
        let undo = make_move(game, mv);

        // Illegal move check (king in check after our move)
        if is_in_check(game, color) {
            unmake_move(game, mv, undo);
            continue;
        }

        legal_move_count += 1;

        // Recursive search
        let score = -alphabeta(game, depth - 1, -beta, -alpha, -color)?;
        unmake_move(game, mv, undo);

        if score > best_score {
            best_score = score;
            best_move = mv;
        }

        alpha = alpha.max(score);

        // Beta cutoff
        if alpha >= beta {
            game.cut += 1;

            // Store in TT
            store_tt_entry(game, hash, depth, best_score, best_move);

            // Update heuristics
            update_heuristics(game, depth, best_move);

            return Ok(best_score);
        }
    }

    // Checkmate/Stalemate check if no legal moves were found
    if legal_move_count == 0 {
        return Ok(if in_check {
            -KING_VALUE + (100 - depth) as i16 // Checkmate
        } else {
            0 // Stalemate
        });
    }

    // Store in TT and return
    store_tt_entry(game, hash, depth, best_score, best_move);

    Ok(best_score)
}

fn store_tt_entry(game: &mut Game, hash: BitBuffer192, depth: i32, score: i16, best_move: KK) {
    let mut hash_result = HashResult::default();
    hash_result.depth = depth as i64;
    hash_result.hit = 1;
    hash_result.h[0] = Guide1 {
        ply: depth as i64,
        score,
        best_move_src: best_move.src,
        best_move_dst: best_move.dst,
        best_move_nxt_dir_idx: 0,
    };
    let priority = depth as i64 * 10 + game.move_counter as i64;
    tt_store(game, hash, hash_result, priority);
}

fn update_heuristics(game: &mut Game, depth: i32, best_move: KK) {
    let d_idx = depth.max(0) as usize;
    if d_idx <= MAX_DEPTH && best_move.score == 0 {
        // Only for non-captures
        // Shift killers and insert new one
        if game.killer_moves[d_idx][0].map_or(true, |k| {
            k.src != best_move.src || k.dst != best_move.dst
        }) {
            game.killer_moves[d_idx][1] = game.killer_moves[d_idx][0];
            game.killer_moves[d_idx][0] = Some(best_move);
        }
        // Increment history score
        game.history_table[best_move.src as usize][best_move.dst as usize] +=
            (depth * depth) as u32;
    }
}
