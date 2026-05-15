//! Quiescence search to avoid horizon effect
//!
//! This module implements quiescence search to extend the search tree
//! for tactical sequences (captures) beyond the nominal depth.

use super::make_unmake::{make_move, unmake_move};
use super::ordering::order_moves;
use crate::error::ChessEngineResult;
use crate::evaluation::*;
use crate::move_gen::*;
use crate::types::*;

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

    // Stand-pat evaluation: if current position is already good enough, return it
    let stand_pat = evaluate_position(game) * (if color > 0 { 1 } else { -1 });

    if stand_pat >= beta {
        return Ok(beta);
    }

    if stand_pat > alpha {
        alpha = stand_pat;
    }

    // Limit depth to avoid runaway tacticals
    if qs_depth >= MAX_QUIESCENCE_DEPTH {
        return Ok(stand_pat);
    }

    // Generate and order capture moves
    let mut moves = generate_pseudo_legal_moves(game, color);
    moves.retain(|m| game.board[m.dst as usize] != 0);

    if moves.is_empty() {
        return Ok(stand_pat);
    }

    // Use depth 0 for ordering in QS (captures only)
    order_moves(game, &mut moves, 0);

    for mv in moves {
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
