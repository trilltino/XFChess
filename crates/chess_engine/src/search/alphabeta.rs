//! Alpha-beta search with negamax (ITERATIVE VERSION - No recursion)
//!
//! This function has been converted from recursive to iterative to eliminate
//! stack overflow issues. It uses an explicit stack to simulate recursive calls.

use super::make_unmake::{make_move, unmake_move, UndoInfo};
use super::ordering::order_moves;
use super::quiescence::quiescence_search;
use crate::constants::*;
use crate::error::{ChessEngineError, ChessEngineResult};
use crate::hash::*;
use crate::move_gen::*;
use crate::types::*;
use futures_lite::future::yield_now;

/// Stack frame for iterative alphabeta search
///
/// Each frame represents one "recursive call" in the original recursive implementation.
/// By using an explicit stack, we eliminate unbounded call stack growth.
struct SearchFrame {
    depth: i32,
    alpha: i16,
    beta: i16,
    color: Color,
    move_index: usize,
    moves: Vec<KK>,
    best_score: i16,
    best_move: Option<KK>,
    in_check: bool,
    made_move: Option<(KK, UndoInfo)>,
    returning_score: Option<i16>,
    extensions_used: i32, // Track cumulative depth extensions to prevent infinite loops
}

/// Alpha-beta search with negamax (ITERATIVE VERSION - No recursion - Async)
///
/// This function has been converted from recursive to iterative to eliminate
/// stack overflow issues. It uses an explicit stack to simulate recursive calls.
///
/// # Errors
///
/// Returns an error if the search algorithm encounters stack corruption or
/// logic errors that prevent proper execution.
pub(crate) async fn alphabeta(
    game: &mut Game,
    depth: i32,
    alpha: i16,
    beta: i16,
    color: Color,
) -> ChessEngineResult<i16> {
    // Stack of search frames (replaces call stack)
    let mut stack: Vec<SearchFrame> = vec![SearchFrame {
        depth,
        alpha,
        beta,
        color,
        move_index: 0,
        moves: Vec::new(),
        best_score: -AB_INF,
        best_move: None,
        in_check: false,
        made_move: None,
        returning_score: None,
        extensions_used: 0, // Start with no extensions
    }];

    use instant::Instant;

    // Time-based yielding to ensure 60 FPS (max 5ms per chunk)
    let mut chunk_start = Instant::now();

    // Main search loop (replaces recursion)
    while let Some(frame) = stack.last_mut() {
        // === PHASE 1: Frame Initialization (first visit) ===
        if frame.moves.is_empty() && frame.move_index == 0 && frame.returning_score.is_none() {
            game.calls += 1;

            // Check more frequently (every 128 nodes) but only yield if time budget exceeded
            if game.calls % 128 == 0 {
                // Yield if we've worked for more than 5ms in this time slice
                if chunk_start.elapsed().as_millis() > 5 {
                    yield_now().await;
                    chunk_start = Instant::now();
                }
            }

            // Base case: depth 0 - do quiescence search
            if frame.depth <= 0 {
                let score = quiescence_search(game, frame.alpha, frame.beta, frame.color)?;
                // Extract depth before popping to avoid borrow checker issues
                let frame_depth = frame.depth;
                let _current_frame = stack
                    .pop()
                    .ok_or_else(|| ChessEngineError::StackUnderflow { depth: frame_depth })?;

                // Return score to parent frame
                if let Some(parent) = stack.last_mut() {
                    parent.returning_score = Some(-score);
                } else {
                    // Root call, return directly
                    return Ok(score);
                }
                continue;
            }

            // Transposition table probe
            let hash = position_hash(&game.board);
            if let Some(cached) = tt_probe(game, &hash) {
                if cached.depth >= frame.depth as i64 {
                    game.tte_hit += 1;
                    if !cached.h.is_empty() && cached.h[0].score != INVALID_SCORE {
                        let score = cached.h[0].score;
                        // Extract depth before popping to avoid borrow checker issues
                        let frame_depth = frame.depth;
                        let _current_frame =
                            stack
                                .pop()
                                .ok_or_else(|| ChessEngineError::StackUnderflow {
                                    depth: frame_depth,
                                })?;

                        if let Some(parent) = stack.last_mut() {
                            parent.returning_score = Some(-score);
                        } else {
                            return Ok(score);
                        }
                        continue;
                    }
                }
            }

            // Check detection and BOUNDED depth extension
            // Prevents infinite extension loops in perpetual check sequences
            frame.in_check = is_in_check(game, frame.color);
            const MAX_EXTENSIONS: i32 = 4; // Allow max 4 depth extensions (8 + 4 = 12 ply max)
            if frame.in_check && frame.extensions_used < MAX_EXTENSIONS {
                frame.depth += 1;
                frame.extensions_used += 1;
            }

            // Generate and order moves
            frame.moves = generate_pseudo_legal_moves(game, frame.color);

            // No moves available - checkmate or stalemate
            if frame.moves.is_empty() {
                let score = if frame.in_check {
                    -KING_VALUE + (100 - frame.depth) as i16 // Checkmate
                } else {
                    0 // Stalemate
                };

                // Extract depth before popping to avoid borrow checker issues
                let frame_depth = frame.depth;
                let _current_frame = stack
                    .pop()
                    .ok_or_else(|| ChessEngineError::StackUnderflow { depth: frame_depth })?;
                if let Some(parent) = stack.last_mut() {
                    parent.returning_score = Some(-score);
                } else {
                    return Ok(score);
                }
                continue;
            }

            order_moves(game, &mut frame.moves);
            frame.best_move = Some(frame.moves[0]);
            continue;
        }

        // === PHASE 2: Process Returning Score from Child ===
        if let Some(child_score) = frame.returning_score.take() {
            // Unmake the move we made before calling child
            if let Some((mv, undo)) = frame.made_move.take() {
                unmake_move(game, mv, undo);
            }

            let score = -child_score; // Negamax negation

            if score > frame.best_score {
                frame.best_score = score;
                if frame.move_index > 0 {
                    frame.best_move = Some(frame.moves[frame.move_index - 1]);
                }
            }

            frame.alpha = frame.alpha.max(score);

            // Beta cutoff
            if frame.alpha >= frame.beta {
                game.cut += 1;

                // Store in TT and return
                let best_move = frame
                    .best_move
                    .ok_or_else(|| ChessEngineError::BestMoveNotSet { depth: frame.depth })?;
                let hash = position_hash(&game.board);
                let mut hash_result = HashResult::default();
                hash_result.depth = frame.depth as i64;
                hash_result.hit = 1;
                hash_result.h[0] = Guide1 {
                    ply: frame.depth as i64,
                    score: frame.best_score,
                    best_move_src: best_move.src,
                    best_move_dst: best_move.dst,
                    best_move_nxt_dir_idx: 0,
                };
                let priority = frame.depth as i64 * 10 + game.move_counter as i64;
                tt_store(game, hash, hash_result, priority);

                let final_score = frame.best_score;
                stack.pop();

                if let Some(parent) = stack.last_mut() {
                    parent.returning_score = Some(-final_score);
                } else {
                    return Ok(final_score);
                }
                continue;
            }

            // Continue to next move
        }

        // === PHASE 3: Try Next Move ===
        if frame.move_index < frame.moves.len() {
            let mv = frame.moves[frame.move_index];
            // Collect values before push (avoids borrow checker issues)
            let child_depth = frame.depth - 1;
            let child_alpha = -frame.beta;
            let child_beta = -frame.alpha;
            let child_color = -frame.color;
            let check_color = frame.color;
            let parent_extensions = frame.extensions_used; // Copy extension count before borrow

            frame.move_index += 1;

            let undo = make_move(game, mv);

            // Illegal move check (king in check after our move)
            if is_in_check(game, check_color) {
                unmake_move(game, mv, undo);
                continue; // Skip this move
            }

            // Store move for unmake after child returns
            frame.made_move = Some((mv, undo));

            // Mutable reference ends when we collect values and push below

            // Push child frame (simulates recursive call)
            stack.push(SearchFrame {
                depth: child_depth,
                alpha: child_alpha,
                beta: child_beta,
                color: child_color,
                move_index: 0,
                moves: Vec::new(),
                best_score: -AB_INF,
                best_move: None,
                in_check: false,
                made_move: None,
                returning_score: None,
                extensions_used: parent_extensions, // Inherit parent's extension count
            });
            continue;
        }

        // === PHASE 4: All Moves Processed - Return Result ===
        // Defensive check: ensure best_move is set before using it
        if frame.best_move.is_none() && !frame.moves.is_empty() {
            // Fallback: use first move if best_move not set
            frame.best_move = Some(frame.moves[0]);
        }

        let best_move = frame
            .best_move
            .ok_or_else(|| ChessEngineError::BestMoveNotSet { depth: frame.depth })?;
        let hash = position_hash(&game.board);
        let mut hash_result = HashResult::default();
        hash_result.depth = frame.depth as i64;
        hash_result.hit = 1;
        hash_result.h[0] = Guide1 {
            ply: frame.depth as i64,
            score: frame.best_score,
            best_move_src: best_move.src,
            best_move_dst: best_move.dst,
            best_move_nxt_dir_idx: 0,
        };
        let priority = frame.depth as i64 * 10 + game.move_counter as i64;
        tt_store(game, hash, hash_result, priority);

        let final_score = frame.best_score;
        stack.pop();

        if let Some(parent) = stack.last_mut() {
            parent.returning_score = Some(-final_score);
        } else {
            // Root level - return final result
            return Ok(final_score);
        }
    }

    // Stack became empty unexpectedly - this should not happen in normal execution
    // Log the error and return a conservative score
    Err(ChessEngineError::SearchError {
        message: format!(
            "alphabeta: stack became empty unexpectedly at depth {}",
            depth
        ),
    })
}
