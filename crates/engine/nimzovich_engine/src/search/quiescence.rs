//! Quiescence search to avoid horizon effect
//!
//! Implements quiescence search with:
//! - Delta pruning: skip captures that can't raise alpha
//! - SEE filtering: skip bad captures statically
//! - Check evasion: generate all moves when in check

use super::make_unmake::{make_move, unmake_move};
use super::ordering::order_moves;
use super::params::SearchParams;
use crate::error::ChessEngineResult;
use crate::evaluation::*;
use crate::move_gen::*;
use crate::types::*;

static SP: SearchParams = SearchParams::sarah_tuned();

/// Maximum quiescence search depth to prevent runaway capture sequences
const MAX_QUIESCENCE_DEPTH: i32 = 4;

/// Quiescence search to avoid horizon effect (Recursive)
pub(crate) fn quiescence_search(
    game: &mut Game,
    alpha: i16,
    beta: i16,
    color: Color,
) -> ChessEngineResult<i16> {
    quiescence_recursive(game, alpha, beta, color, 0)
}

fn quiescence_recursive(
    game: &mut Game,
    mut alpha: i16,
    beta: i16,
    color: Color,
    qs_depth: i32,
) -> ChessEngineResult<i16> {
    game.calls += 1;

    // Stand-pat evaluation
    let stand_pat = evaluate_position(game) * (if color > 0 { 1 } else { -1 });

    if stand_pat >= beta {
        return Ok(beta);
    }

    if stand_pat > alpha {
        alpha = stand_pat;
    }

    // Limit depth
    if qs_depth >= MAX_QUIESCENCE_DEPTH {
        return Ok(stand_pat);
    }

    let in_check = is_in_check(game, color);

    // Generate moves: captures only unless in check
    let mut moves = generate_pseudo_legal_moves(game, color);
    if !in_check {
        moves.retain(|m| game.board[m.dst as usize] != 0 || (m.nxt_dir_idx >> 4) != 0);
    }

    if moves.is_empty() {
        return Ok(stand_pat);
    }

    // Order moves (depth 0 for QS)
    order_moves(game, &mut moves, 0);

    for mv in moves {
        // Victim must be read BEFORE make_move (afterwards the destination
        // holds the moving piece). FIGURE_VALUE[0] = 0 for non-captures.
        let victim = game.board[mv.dst as usize];

        if !in_check {
            // Delta pruning: if even capturing the piece doesn't raise alpha,
            // skip. i32 math — the sum overflows i16 when alpha is near ±AB_INF.
            let captured_val = crate::constants::FIGURE_VALUE[victim.abs() as usize];
            let promo_bonus: i32 = if (mv.nxt_dir_idx >> 4) != 0 { 800 } else { 0 };
            let best_case = stand_pat as i32 + captured_val as i32 + promo_bonus + SP.qdelta_margin;
            if best_case <= alpha as i32 {
                continue;
            }

            // SEE filter: don't search losing captures in quiescence.
            if !crate::see::see(game, mv, 0) {
                continue;
            }
        }

        let undo = make_move(game, mv);

        if is_in_check(game, color) {
            unmake_move(game, mv, undo);
            continue;
        }

        let score = -quiescence_recursive(game, -beta, -alpha, -color, qs_depth + 1)?;
        unmake_move(game, mv, undo);

        if score >= beta {
            return Ok(beta);
        }

        if score > alpha {
            alpha = score;
        }
    }

    Ok(alpha)
}
