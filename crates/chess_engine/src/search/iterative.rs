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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::new_game;

    // Helper to run async tests in sync context
    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        futures_lite::future::block_on(f)
    }

    #[test]
    fn test_find_best_move_starting_position() {
        let mut game = new_game();

        // Find best move with short time limit
        let best_move = block_on(find_best_move(&mut game, 0.1, COLOR_WHITE));

        // Should return a valid move (non-zero src/dst)
        assert!(
            best_move.src != 0 || best_move.dst != 0,
            "Should find a move"
        );

        // Source should be a white piece
        let src = best_move.src as usize;
        assert!(src < 64, "Source should be valid position");
        assert!(game.board[src] > 0, "Source should be a white piece");
    }

    #[test]
    fn test_find_best_move_returns_legal_move() {
        let mut game = new_game();

        let best_move = block_on(find_best_move(&mut game, 0.1, COLOR_WHITE));

        // Verify the move is in the list of legal moves
        let moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);
        let found = moves
            .iter()
            .any(|m| m.src == best_move.src as i8 && m.dst == best_move.dst as i8);

        assert!(found, "Best move should be a legal pseudo-legal move");
    }

    #[test]
    fn test_iterative_deepening_updates_stats() {
        let mut game = new_game();

        // Stats should be zero initially
        game.calls = 0;
        game.cut = 0;

        let _ = block_on(iterative_deepening(&mut game, 0.1, COLOR_WHITE));

        // After search, stats should be updated
        assert!(game.calls > 0, "Should have made search calls");
    }

    #[test]
    fn test_find_best_move_for_black() {
        let mut game = new_game();

        // Make a move for white first
        use super::super::make_unmake::make_move;
        let moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);
        if let Some(mv) = moves.first() {
            make_move(&mut game, *mv);
        }

        // Now find best move for black
        let best_move = block_on(find_best_move(&mut game, 0.1, COLOR_BLACK));

        // Should return a valid move
        assert!(
            best_move.src != 0 || best_move.dst != 0,
            "Should find a move for black"
        );
    }

    #[test]
    fn test_move_default_values() {
        let m = Move::default();

        assert_eq!(m.src, 0);
        assert_eq!(m.dst, 0);
        assert_eq!(m.state, STATE_PLAYING);
        assert_eq!(m.checkmate_in, 0);
    }
}
