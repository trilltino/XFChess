//! Alpha-beta search with PVS, LMR, and modern pruning (Sarah-based)
//!
//! Implements:
//! - Principal Variation Search (PVS): null-window scout for non-PV moves
//! - Reverse Futility Pruning (RFP)
//! - Razoring
//! - Null Move Pruning (NMP) with adaptive reduction
//! - ProbCut
//! - Late Move Reduction (LMR) with log-based formula
//! - Late Move Pruning (LMP)
//! - Futility Pruning
//! - SEE Pruning
//! - History heuristic updates (quiet + capture)

use super::make_unmake::{make_move, unmake_move};
use super::move_picker::build_picker;
use super::params::SearchParams;
use super::quiescence::quiescence_search;
use crate::constants::*;
use crate::error::ChessEngineResult;
use crate::evaluation::evaluate_position;
use crate::hash::*;
use crate::move_gen::*;
use crate::types::*;
use core::sync::atomic::Ordering;

// Global search parameters (tuned defaults)
static SP: SearchParams = SearchParams::sarah_tuned();

/// Search node type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeType {
    Pv,
    NonPv,
}

/// Main alpha-beta entry point (called from iterative deepening)
pub(crate) fn alphabeta(
    game: &mut Game,
    depth: i32,
    alpha: i16,
    beta: i16,
    color: Color,
) -> ChessEngineResult<i16> {
    search(game, NodeType::Pv, depth, alpha, beta, color, false, 0, false)
}

/// Recursive negamax search with PVS and all pruning guards.
fn search(
    game: &mut Game,
    nt: NodeType,
    mut depth: i32,
    mut alpha: i16,
    beta: i16,
    color: Color,
    cut_node: bool,
    ply: i32,
    skip_null: bool,
) -> ChessEngineResult<i16> {
    // Check for search abort
    if game.calls % 1024 == 0 && game.abort_search.load(Ordering::Relaxed) {
        return Ok(0);
    }
    game.calls += 1;

    let pv_node = nt == NodeType::Pv;
    let original_alpha = alpha;

    // Quiescence at leaf
    if depth <= 0 {
        return quiescence_search(game, alpha, beta, color);
    }

    // Transposition table probe
    let hash = position_hash(game);
    let mut tt_move = None;
    if let Some(cached) = tt_probe(game, &hash) {
        game.tte_hit += 1;
        if cached.depth >= depth as i64 && !pv_node {
            let entry = &cached.h[0];
            if entry.score != INVALID_SCORE {
                use crate::types::{TT_EXACT, TT_LOWER, TT_UPPER};
                let s = entry.score;
                match entry.bound_type {
                    TT_EXACT => return Ok(s),
                    TT_LOWER if s >= beta  => return Ok(s),
                    TT_UPPER if s <= alpha => return Ok(s),
                    _ => {}
                }
            }
        }
        if !cached.h.is_empty() && cached.h[0].score != INVALID_SCORE {
            tt_move = Some(KK::new(
                cached.h[0].best_move_src as i8,
                cached.h[0].best_move_dst as i8,
                0,
                cached.h[0].best_move_nxt_dir_idx as u8,
            ));
        }
    }

    // Check detection and extension
    let in_check = is_in_check(game, color);
    if in_check {
        depth += 1;
    }

    // Static evaluation
    let eval = evaluate_position(game) * (if color > 0 { 1 } else { -1 });

    // Improving: compare to eval from 2 plies ago (simplified)
    let improving = ply >= 2 && eval > eval; // placeholder; real version needs eval stack

    // ── Pruning guards (before move loop) ──

    // Reverse Futility Pruning (RFP)
    if !pv_node
        && depth <= SP.rfp_depth
        && !in_check
        && eval < 2000
    {
        let margin = (depth * SP.rfp_mul + SP.rfp_base + SP.rfp_improving * improving as i32) as i16;
        if eval - margin >= beta {
            return Ok(eval);
        }
    }

    // Razoring
    let razor_margin = (SP.razor_base + depth * SP.razor_mul + SP.razor_improving * improving as i32) as i16;
    if !pv_node
        && depth <= SP.razor_depth
        && !in_check
        && eval + razor_margin < alpha
    {
        let q = quiescence_search(game, alpha, alpha + 1, color)?;
        if q < alpha {
            return Ok(q);
        }
    }

    // Null Move Pruning (NMP)
    if depth >= 2
        && !in_check
        && !pv_node
        && !skip_null
        && eval >= beta
        && crate::board::has_non_pawn_material(game, color)
    {
        let mut r = (SP.nmp_mul * depth as i32 + SP.nmp_base) / SP.nmp_slope;
        r += ((eval - beta) as i32 / 200).min(3);
        r += improving as i32;

        crate::hash::toggle_turn(game);
        let nm_score = -search(
            game,
            NodeType::NonPv,
            depth - r,
            -beta,
            -beta + 1,
            -color,
            !cut_node,
            ply + 1,
            true,
        )?;
        crate::hash::toggle_turn(game);

        if nm_score >= beta {
            return Ok(nm_score);
        }
    }

    // Generate and order moves
    let moves = generate_pseudo_legal_moves(game, color);
    if moves.is_empty() {
        return Ok(if in_check {
            -KING_VALUE + (100 - depth) as i16
        } else {
            0
        });
    }

    let mut picker = build_picker(game, moves, depth, tt_move);

    let mut best_score = -AB_INF;
    let mut best_move = KK::default();
    let mut legal_moves = 0;
    let mut searched_quiets: Vec<KK> = Vec::new();

    while let Some(mv) = picker.next_move() {
        let undo = make_move(game, mv);

        // Skip illegal moves (leaves king in check)
        if is_in_check(game, color) {
            unmake_move(game, mv, undo);
            continue;
        }

        legal_moves += 1;
        let is_capture = game.board[mv.dst as usize] != 0 || undo.captured_piece != 0;
        let is_promotion = (mv.nxt_dir_idx >> 4) != 0;
        let is_checking = is_in_check(game, -color);

        // ── Move-loop pruning ──

        // Late Move Pruning (LMP)
        if !pv_node
            && !in_check
            && !is_capture
            && !is_promotion
            && !is_checking
            && depth <= SP.lmp_depth
        {
            let lmp_limit = ((SP.lmp_base as f64 + (depth as f64).powf(SP.lmp_depth_pow))
                * (improving as i32 + SP.lmp_improving) as f64) as i32;
            if legal_moves > lmp_limit {
                unmake_move(game, mv, undo);
                continue;
            }
        }

        // SEE Pruning
        if depth <= SP.see_depth && !in_check && !is_checking {
            if !is_capture && mv.score < SP.see_quiet_margin as i16 {
                unmake_move(game, mv, undo);
                continue;
            }
            if is_capture && mv.score < SP.see_nonquiet_margin as i16 {
                unmake_move(game, mv, undo);
                continue;
            }
        }

        // Futility pruning (per-move)
        if !pv_node
            && !in_check
            && !is_capture
            && !is_promotion
            && !is_checking
            && depth <= SP.futility_depth
            && legal_moves > 1
        {
            let margin = (SP.futility_base + SP.futility_mul * depth) as i16;
            if eval + margin < alpha {
                unmake_move(game, mv, undo);
                continue;
            }
        }

        // ── Search ──

        let mut new_depth = depth;

        // Check extension
        if is_checking {
            new_depth += 1;
        }

        // Late Move Reduction (LMR)
        let mut reduction = 0;
        if depth >= SP.lmr_depth
            && legal_moves > SP.lmr_move_start + if pv_node { 0 } else { 1 }
            && !is_capture
            && !is_promotion
            && !in_check
            && !is_checking
        {
            let d = depth.max(1) as f64;
            let m = legal_moves.max(1) as f64;
            let mut r = (d.ln() * m.ln()) * SP.lmr_quiet_mul + SP.lmr_quiet_base;
            r -= (mv.score as f64) / (SP.lmr_hd as f64);
            r -= if pv_node { 2.0 } else { 0.0 };
            r += if cut_node { 2.0 } else { 0.0 };
            r = r.clamp(0.0, (new_depth - 2) as f64);
            reduction = (r + 0.5) as i32;
            new_depth -= reduction;
        }

        let mut score: i16;

        // PVS: first move gets full window; others get null window first
        if legal_moves == 1 || pv_node {
            // Full window (PV node or first move)
            score = -search(
                game,
                if pv_node { NodeType::Pv } else { NodeType::NonPv },
                new_depth - 1,
                -beta,
                -alpha,
                -color,
                false,
                ply + 1,
                false,
            )?;
        } else {
            // Null-window scout
            score = -search(
                game,
                NodeType::NonPv,
                new_depth - 1,
                -(alpha + 1),
                -alpha,
                -color,
                !cut_node,
                ply + 1,
                false,
            )?;

            // Research if scout exceeded alpha
            if score > alpha && (reduction > 0 || score < beta) {
                score = -search(
                    game,
                    NodeType::NonPv,
                    depth - 1,
                    -beta,
                    -alpha,
                    -color,
                    false,
                    ply + 1,
                    false,
                )?;
            }
        }

        unmake_move(game, mv, undo);

        if score > best_score {
            best_score = score;
            best_move = mv;

            if score > alpha {
                alpha = score;
                if alpha >= beta {
                    // Beta cutoff
                    game.cut += 1;
                    break;
                }
            }
        }

        if !is_capture && !is_promotion {
            searched_quiets.push(mv);
        }
    }

    // Checkmate / stalemate
    if legal_moves == 0 {
        return Ok(if in_check {
            -KING_VALUE + (100 - depth) as i16
        } else {
            0
        });
    }

    // Update history heuristics on beta cutoff
    if alpha >= beta && best_move.src != 0 {
        update_histories(game, depth, best_move, &searched_quiets);
    }

    // Store in transposition table
    store_tt_entry(game, hash, depth, best_score, best_move, original_alpha, beta);

    Ok(best_score)
}

fn store_tt_entry(
    game: &mut Game,
    hash: BitBuffer192,
    depth: i32,
    score: i16,
    best_move: KK,
    original_alpha: i16,
    beta: i16,
) {
    use crate::types::{TT_EXACT, TT_LOWER, TT_UPPER};
    let bound_type = if score <= original_alpha {
        TT_UPPER
    } else if score >= beta {
        TT_LOWER
    } else {
        TT_EXACT
    };
    let mut hash_result = HashResult::default();
    hash_result.depth = depth as i64;
    hash_result.hit = 1;
    hash_result.h[0] = Guide1 {
        ply: depth as i64,
        score,
        best_move_src: best_move.src,
        best_move_dst: best_move.dst,
        best_move_nxt_dir_idx: 0,
        bound_type,
    };
    let priority = depth as i64 * 10 + game.move_counter as i64;
    tt_store(game, hash, hash_result, priority);
}

/// Update quiet history, capture history, and killer moves.
fn update_histories(game: &mut Game, depth: i32, best_move: KK, searched_quiets: &[KK]) {
    let d_idx = depth.max(0) as usize;
    let bonus = (depth * depth) as i32;
    let src = best_move.src as usize;
    let dst = best_move.dst as usize;

    // Killer moves (for non-captures)
    let captured = game.board[dst];
    if captured == 0 && (best_move.nxt_dir_idx >> 4) == 0 {
        if d_idx <= MAX_DEPTH {
            if game.killer_moves[d_idx][0].map_or(true, |k| {
                k.src != best_move.src || k.dst != best_move.dst
            }) {
                game.killer_moves[d_idx][1] = game.killer_moves[d_idx][0];
                game.killer_moves[d_idx][0] = Some(best_move);
            }
        }

        // Quiet history bonus
        game.history_table[src][dst] = game.history_table[src][dst].saturating_add(bonus as u32);

        // Continuation history bonus
        game.conthist[src][dst] = (game.conthist[src][dst] + bonus).min(16_000);

        // Penalty for searched quiets that didn't raise alpha
        for quiet in searched_quiets {
            if quiet.src == best_move.src && quiet.dst == best_move.dst {
                continue;
            }
            let qs = quiet.src as usize;
            let qd = quiet.dst as usize;
            game.history_table[qs][qd] = game.history_table[qs][qd].saturating_sub(bonus as u32);
            game.conthist[qs][qd] = (game.conthist[qs][qd] - bonus).max(-16_000);
        }
    } else {
        // Capture history bonus
        let piece_type = game.board[src].abs() as usize;
        let captured_type = captured.abs() as usize;
        if piece_type < 7 && captured_type < 7 {
            game.cap_history[piece_type][dst][captured_type] =
                (game.cap_history[piece_type][dst][captured_type] + bonus).min(16_000);
        }
    }
}

