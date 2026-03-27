//! Move making and unmaking for search
//!
//! Provides functions to make and unmake moves during search, with undo
//! information to restore the board state.

use crate::types::*;

/// Information needed to undo a move
pub(crate) struct UndoInfo {
    pub captured_piece: i8,
    pub from_square_piece: i8,
}

/// Make a move on the board (returns undo information)
pub(crate) fn make_move(game: &mut Game, mv: KK) -> UndoInfo {
    let undo = UndoInfo {
        captured_piece: game.board[mv.dst as usize],
        from_square_piece: game.board[mv.src as usize],
    };

    game.board[mv.dst as usize] = game.board[mv.src as usize];
    game.board[mv.src as usize] = 0;
    game.move_counter += 1;

    undo
}

/// Unmake a move on the board
pub(crate) fn unmake_move(game: &mut Game, mv: KK, undo: UndoInfo) {
    game.board[mv.src as usize] = undo.from_square_piece;
    game.board[mv.dst as usize] = undo.captured_piece;
    game.move_counter -= 1;
}
