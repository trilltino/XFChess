//! Quiescence search to avoid horizon effect
//!
//! **Stack Overflow Fix**: Completely eliminated recursion by using explicit stack frames.
//! This prevents stack overflow in ANY capture sequence, regardless of depth or branching factor.

use super::make_unmake::{make_move, unmake_move, UndoInfo};
use super::ordering::order_moves;
use crate::error::{ChessEngineError, ChessEngineResult};
use crate::evaluation::*;
use crate::move_gen::*;
use crate::types::*;

/// Maximum quiescence search depth to prevent runaway capture sequences
/// REDUCED from 8 to 2 to prevent stack overflow in deep tactical lines
const MAX_QUIESCENCE_DEPTH: i32 = 2;

/// Stack frame for iterative quiescence search
///
/// Each frame represents one "recursive call" in the original recursive implementation.
/// By using an explicit stack, we eliminate unbounded call stack growth.
struct QuiescenceFrame {
    alpha: i16,
    beta: i16,
    color: Color,
    qs_depth: i32,
    stand_pat: i16,
    moves: Vec<KK>,
    move_index: usize,
    best_score: i16,
    made_move: Option<(KK, UndoInfo)>,
    returning_score: Option<i16>,
}

/// Quiescence search to avoid horizon effect - FULLY ITERATIVE
///
/// **Stack Overflow Fix**: Completely eliminated recursion by using explicit stack frames.
/// This prevents stack overflow in ANY capture sequence, regardless of depth or branching factor.
///
/// # Errors
///
/// Returns an error if the search algorithm encounters stack corruption or
/// logic errors that prevent proper execution.
pub(crate) fn quiescence_search(
    game: &mut Game,
    alpha: i16,
    beta: i16,
    color: Color,
) -> ChessEngineResult<i16> {
    // Stack of search frames (replaces call stack)
    let mut stack: Vec<QuiescenceFrame> = vec![QuiescenceFrame {
        alpha,
        beta,
        color,
        qs_depth: 0,
        stand_pat: 0,
        moves: Vec::new(),
        move_index: 0,
        best_score: alpha,
        made_move: None,
        returning_score: None,
    }];

    // Main search loop (replaces recursion)
    while let Some(frame) = stack.last_mut() {
        // === PHASE 1: Frame Initialization (first visit) ===
        if frame.moves.is_empty() && frame.returning_score.is_none() {
            game.calls += 1;

            // Depth limit check - prevent excessive search
            if frame.qs_depth >= MAX_QUIESCENCE_DEPTH {
                let eval = evaluate_position(game) * (if frame.color > 0 { 1 } else { -1 });
                stack.pop();
                if let Some(parent) = stack.last_mut() {
                    parent.returning_score = Some(-eval);
                } else {
                    return Ok(eval);
                }
                continue;
            }

            // Stand-pat evaluation
            frame.stand_pat = evaluate_position(game) * (if frame.color > 0 { 1 } else { -1 });

            // Beta cutoff
            if frame.stand_pat >= frame.beta {
                let beta = frame.beta;
                stack.pop();
                if let Some(parent) = stack.last_mut() {
                    parent.returning_score = Some(-beta);
                } else {
                    return Ok(beta);
                }
                continue;
            }

            // Update alpha with stand-pat
            if frame.stand_pat > frame.alpha {
                frame.alpha = frame.stand_pat;
                frame.best_score = frame.stand_pat;
            }

            // Generate capture moves only
            frame.moves = generate_pseudo_legal_moves(game, frame.color);
            frame.moves.retain(|m| game.board[m.dst as usize] != 0);

            // No captures available - return stand-pat
            if frame.moves.is_empty() {
                let stand_pat = frame.stand_pat;
                stack.pop();
                if let Some(parent) = stack.last_mut() {
                    parent.returning_score = Some(-stand_pat);
                } else {
                    return Ok(stand_pat);
                }
                continue;
            }

            order_moves(game, &mut frame.moves);
        }

        // === PHASE 2: Process Child Return ===
        if let Some(child_score) = frame.returning_score.take() {
            // Unmake the move that was tried
            if let Some((mv, undo)) = frame.made_move.take() {
                unmake_move(game, mv, undo);
            }

            // Negamax: negate child's score
            let score = -child_score;

            // Beta cutoff
            if score >= frame.beta {
                let beta = frame.beta;
                stack.pop();
                if let Some(parent) = stack.last_mut() {
                    parent.returning_score = Some(-beta);
                } else {
                    return Ok(beta);
                }
                continue;
            }

            // Update alpha
            if score > frame.alpha {
                frame.alpha = score;
                frame.best_score = score;
            }
        }

        // === PHASE 3: Try Next Move ===
        if frame.move_index < frame.moves.len() {
            let mv = frame.moves[frame.move_index];

            // Collect values before push (avoids borrow checker issues)
            let child_alpha = -frame.beta;
            let child_beta = -frame.alpha;
            let child_color = -frame.color;
            let child_depth = frame.qs_depth + 1;
            let check_color = frame.color;

            frame.move_index += 1;

            let undo = make_move(game, mv);

            // Illegal move check (leaves king in check)
            if is_in_check(game, check_color) {
                unmake_move(game, mv, undo);
                continue; // Skip this move
            }

            // Store move for unmake after child returns
            frame.made_move = Some((mv, undo));

            // Push child frame (simulates recursive call)
            stack.push(QuiescenceFrame {
                alpha: child_alpha,
                beta: child_beta,
                color: child_color,
                qs_depth: child_depth,
                stand_pat: 0,
                moves: Vec::new(),
                move_index: 0,
                best_score: child_alpha,
                made_move: None,
                returning_score: None,
            });
            continue;
        }

        // === PHASE 4: All Moves Processed - Return Result ===
        let best_score = frame.best_score;
        stack.pop();
        if let Some(parent) = stack.last_mut() {
            parent.returning_score = Some(-best_score);
        } else {
            return Ok(best_score);
        }
    }

    // Stack became empty unexpectedly - this should not happen in normal execution
    // Return an error with context
    Err(ChessEngineError::SearchError {
        message: format!(
            "quiescence_search: stack became empty unexpectedly at depth {}",
            MAX_QUIESCENCE_DEPTH
        ),
    })
}
