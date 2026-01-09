//! Game state queries and AI move generation
//!
//! Functions for querying game state and generating AI moves.

use crate::constants::*;
use crate::move_gen::*;
use crate::search::*;
use crate::types::*;

/// Get AI's response to current position
///
/// # Arguments
///
/// * `game` - The game state
/// * `color` - The color to move (1 for White, -1 for Black)
pub async fn reply(game: &mut Game, color: i64) -> Move {
    find_best_move(game, game.secs_per_move, color).await
}

/// Get current game state (playing, checkmate, stalemate)
pub fn get_game_state(game: &mut Game, color: Color) -> i32 {
    let in_check = is_in_check(game, color);
    let has_legal_moves = has_any_legal_move(game, color);

    if !has_legal_moves {
        if in_check {
            STATE_CHECKMATE
        } else {
            STATE_STALEMATE
        }
    } else {
        STATE_PLAYING
    }
}

fn has_any_legal_move(game: &mut Game, color: Color) -> bool {
    let moves = generate_pseudo_legal_moves(game, color);

    for mv in moves {
        let captured = game.board[mv.dst as usize];
        game.board[mv.dst as usize] = game.board[mv.src as usize];
        game.board[mv.src as usize] = 0;

        let legal = !is_in_check(game, color);

        game.board[mv.src as usize] = game.board[mv.dst as usize];
        game.board[mv.dst as usize] = captured;

        if legal {
            return true;
        }
    }

    false
}
