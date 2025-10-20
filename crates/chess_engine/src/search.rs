//! Alpha-beta search with iterative deepening
//!
//! This module implements the core AI search using:
//! - Negamax variant of alpha-beta pruning (ITERATIVE - no recursion)
//! - Iterative deepening for time management
//! - Transposition table for caching
//! - Move ordering for better pruning
//!
//! **Stack Overflow Fix**: Converted from recursive to iterative implementation
//! using explicit stack frames. This eliminates unbounded recursion and prevents
//! stack overflow at any search depth.

use super::types::*;
use super::constants::*;
use super::board::*;
use super::move_gen::*;
use super::evaluation::*;
use super::hash::*;
use std::time::Instant;

/// Maximum quiescence search depth to prevent runaway capture sequences
const MAX_QUIESCENCE_DEPTH: i32 = 8;

/// Make a move on the board (returns undo information)
struct UndoInfo {
    captured_piece: i8,
    from_square_piece: i8,
}

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
}

fn make_move(game: &mut Game, mv: KK) -> UndoInfo {
    let undo = UndoInfo {
        captured_piece: game.board[mv.dst as usize],
        from_square_piece: game.board[mv.src as usize],
    };

    game.board[mv.dst as usize] = game.board[mv.src as usize];
    game.board[mv.src as usize] = 0;
    game.move_counter += 1;

    undo
}

fn unmake_move(game: &mut Game, mv: KK, undo: UndoInfo) {
    game.board[mv.src as usize] = undo.from_square_piece;
    game.board[mv.dst as usize] = undo.captured_piece;
    game.move_counter -= 1;
}

/// Order moves for better alpha-beta pruning
fn order_moves(game: &Game, moves: &mut [KK]) {
    for mv in moves.iter_mut() {
        let mut score = 0i16;

        // Captures are good
        let captured = game.board[mv.dst as usize];
        if captured != 0 {
            let attacker_value = FIGURE_VALUE[game.board[mv.src as usize].abs() as usize];
            let victim_value = FIGURE_VALUE[captured.abs() as usize];
            // MVV-LVA: Most Valuable Victim - Least Valuable Attacker
            score += victim_value * 10 - attacker_value;
        }

        // Center control bonus
        let (col, row) = pos_to_square(mv.dst);
        let center_dist = ((col - 3).abs() + (row - 3).abs()) as i16;
        score += (8 - center_dist) * 5;

        mv.score = score;
    }

    // Sort moves by score (descending)
    moves.sort_by(|a, b| b.score.cmp(&a.score));
}

/// Alpha-beta search with negamax (ITERATIVE VERSION - No recursion)
///
/// This function has been converted from recursive to iterative to eliminate
/// stack overflow issues. It uses an explicit stack to simulate recursive calls.
fn alphabeta(
    game: &mut Game,
    depth: i32,
    alpha: i16,
    beta: i16,
    color: Color,
) -> i16 {
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
    }];

    // Main search loop (replaces recursion)
    while let Some(frame) = stack.last_mut() {
        // === PHASE 1: Frame Initialization (first visit) ===
        if frame.moves.is_empty() && frame.move_index == 0 && frame.returning_score.is_none() {
            game.calls += 1;

            // Base case: depth 0 - do quiescence search
            if frame.depth <= 0 {
                let score = quiescence_search(game, frame.alpha, frame.beta, frame.color);
                let _current_frame = stack.pop().expect("stack should not be empty");

                // Return score to parent frame
                if let Some(parent) = stack.last_mut() {
                    parent.returning_score = Some(-score);
                } else {
                    // Root call, return directly
                    return score;
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
                        let _current_frame = stack.pop().expect("stack should not be empty");

                        if let Some(parent) = stack.last_mut() {
                            parent.returning_score = Some(-score);
                        } else {
                            return score;
                        }
                        continue;
                    }
                }
            }

            // Check detection and depth extension
            frame.in_check = is_in_check(game, frame.color);
            if frame.in_check {
                frame.depth += 1;
            }

            // Generate and order moves
            frame.moves = generate_pseudo_legal_moves(game, frame.color);

            // No moves available - checkmate or stalemate
            if frame.moves.is_empty() {
                let score = if frame.in_check {
                    -KING_VALUE + (100 - frame.depth) as i16  // Checkmate
                } else {
                    0  // Stalemate
                };

                let _current_frame = stack.pop().expect("stack should not be empty");
                if let Some(parent) = stack.last_mut() {
                    parent.returning_score = Some(-score);
                } else {
                    return score;
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

            let score = -child_score;  // Negamax negation

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
                let best_move = frame.best_move.expect("best_move should be set");
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
                    return final_score;
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

            frame.move_index += 1;

            let undo = make_move(game, mv);

            // Illegal move check (king in check after our move)
            if is_in_check(game, check_color) {
                unmake_move(game, mv, undo);
                continue;  // Skip this move
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
            });
            continue;
        }

        // === PHASE 4: All Moves Processed - Return Result ===
        let best_move = frame.best_move.expect("best_move should be set");
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
            return final_score;
        }
    }

    // Should never reach here
    panic!("alphabeta: stack became empty unexpectedly");
}

/// Quiescence search to avoid horizon effect (with depth limit)
///
/// **Stack Overflow Fix**: Added explicit depth limit to prevent unbounded recursion
/// in long capture sequences (e.g., queen sacrifice leading to 10+ recaptures).
fn quiescence_search(
    game: &mut Game,
    alpha: i16,
    beta: i16,
    color: Color,
) -> i16 {
    quiescence_search_impl(game, alpha, beta, color, 0)
}

/// Internal quiescence search with depth tracking
fn quiescence_search_impl(
    game: &mut Game,
    mut alpha: i16,
    beta: i16,
    color: Color,
    qs_depth: i32,
) -> i16 {
    game.calls += 1;

    // Depth limit check - prevent runaway recursion
    if qs_depth >= MAX_QUIESCENCE_DEPTH {
        return evaluate_position(game) * (if color > 0 { 1 } else { -1 });
    }

    let stand_pat = evaluate_position(game) * (if color > 0 { 1 } else { -1 });

    if stand_pat >= beta {
        return beta;
    }

    if stand_pat > alpha {
        alpha = stand_pat;
    }

    // Only search captures
    let mut moves = generate_pseudo_legal_moves(game, color);
    moves.retain(|m| game.board[m.dst as usize] != 0);

    order_moves(game, &mut moves);

    for mv in moves {
        let undo = make_move(game, mv);

        if is_in_check(game, color) {
            unmake_move(game, mv, undo);
            continue;
        }

        // Recursive call with incremented depth
        let score = -quiescence_search_impl(game, -beta, -alpha, -color, qs_depth + 1);

        unmake_move(game, mv, undo);

        if score >= beta {
            return beta;
        }

        if score > alpha {
            alpha = score;
        }
    }

    alpha
}

/// Iterative deepening search
pub fn iterative_deepening(game: &mut Game, max_time_secs: f32, color: Color) -> Move {
    let start_time = Instant::now();
    let mut best_move = Move::default();
    let mut best_score = LOWEST_SCORE as i16;

    // Reset statistics
    game.calls = 0;
    game.cut = 0;
    game.tte_hit = 0;

    for depth in 1..=MAX_DEPTH {
        let score = alphabeta(game, depth as i32, -AB_INF, AB_INF, color);

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
pub fn find_best_move(game: &mut Game, think_time: f32, color: Color) -> Move {
    iterative_deepening(game, think_time, color)
}
