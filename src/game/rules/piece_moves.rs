//! Chess piece movement rules - Pure move validation logic
//!
//! Implements standard chess movement rules for all piece types using pure functions.
//! This design enables easy unit testing and potential integration with the chess engine
//! in `reference/chess_engine/move_gen.rs`.
//!
//! # Implemented Rules
//!
//! ✅ Basic movement for all pieces (pawn, knight, bishop, rook, queen, king)
//! ✅ Pawn double-move from starting position
//! ✅ Pawn diagonal capture
//! ✅ Path-clear validation for sliding pieces (bishop, rook, queen)
//! ❌ En passant (TODO - see `reference/chess_engine/` for implementation)
//! ❌ Castling (TODO - requires king and rook movement tracking)
//! ❌ Check/checkmate detection (see `game_logic.rs` TODO)
//!
//! # Architecture
//!
//! - **Pure functions**: No ECS dependencies, only piece type and board state
//! - **Snapshot validation**: BoardState provides current piece positions
//! - **No side effects**: Functions return bool, never mutate state
//!
//! # Reference
//!
//! Movement algorithms reference:
//! - `reference/chess_engine/move_gen.rs` - Optimized move generation with bitboards
//! - `reference/bevy-3d-chess/game_logic.rs` - Alternative ECS-integrated approach
//!
//! The chess engine's implementation uses bitboards for 2-5x faster move generation,
//! which could be integrated for AI opponent move calculation.

use crate::rendering::pieces::{PieceType, PieceColor};
use super::board_state::BoardState;

/// Check if a move is valid for a given piece type
pub fn is_valid_move(
    piece_type: PieceType,
    piece_color: PieceColor,
    from: (u8, u8),
    to: (u8, u8),
    board_state: &BoardState,
    has_moved: bool,
) -> bool {
    // Can't move to the same square
    if from == to {
        return false;
    }

    // Can't move off the board
    if to.0 > 7 || to.1 > 7 {
        return false;
    }

    // Can't capture your own pieces
    if let Some(target_color) = board_state.get_piece_color(to) {
        if target_color == piece_color {
            return false;
        }
    }

    match piece_type {
        PieceType::Pawn => is_valid_pawn_move(from, to, piece_color, board_state, has_moved),
        PieceType::Knight => is_valid_knight_move(from, to),
        PieceType::Bishop => is_valid_bishop_move(from, to, board_state),
        PieceType::Rook => is_valid_rook_move(from, to, board_state),
        PieceType::Queen => is_valid_queen_move(from, to, board_state),
        PieceType::King => is_valid_king_move(from, to, has_moved),
    }
}

/// Get all possible moves for a piece
pub fn get_possible_moves(
    piece_type: PieceType,
    piece_color: PieceColor,
    position: (u8, u8),
    board_state: &BoardState,
    has_moved: bool,
) -> Vec<(u8, u8)> {
    let mut moves = Vec::new();

    for x in 0..8 {
        for y in 0..8 {
            let to = (x, y);
            if is_valid_move(piece_type, piece_color, position, to, board_state, has_moved) {
                moves.push(to);
            }
        }
    }

    moves
}

fn is_valid_pawn_move(
    from: (u8, u8),
    to: (u8, u8),
    color: PieceColor,
    board_state: &BoardState,
    has_moved: bool,
) -> bool {
    // Position tuple is (rank, file) where rank is vertical (0-7) and file is horizontal (0-7)
    // Pawns move forward in rank direction: white moves from low rank to high rank (+1)
    let direction = match color {
        PieceColor::White => 1i8,
        PieceColor::Black => -1i8,
    };

    let from_rank = from.0 as i8;
    let from_file = from.1 as i8;
    let to_rank = to.0 as i8;
    let to_file = to.1 as i8;

    let rank_diff = to_rank - from_rank;
    let file_diff = to_file - from_file;

    // Forward move (same file, advance one rank)
    if file_diff == 0 && rank_diff == direction {
        return board_state.is_empty(to);
    }

    // Double move from starting position
    // White pawns start at rank 1, black pawns at rank 6
    let starting_rank = match color {
        PieceColor::White => 1,
        PieceColor::Black => 6,
    };

    if file_diff == 0 && rank_diff == 2 * direction && !has_moved && from.0 == starting_rank {
        let intermediate = ((from.0 as i8 + direction) as u8, from.1);
        return board_state.is_empty(intermediate) && board_state.is_empty(to);
    }

    // Capture diagonally (one file sideways, one rank forward)
    if file_diff.abs() == 1 && rank_diff == direction {
        if let Some(target_color) = board_state.get_piece_color(to) {
            return target_color != color;
        }
        // TODO: En passant
    }

    false
}

fn is_valid_knight_move(from: (u8, u8), to: (u8, u8)) -> bool {
    let dx = (to.0 as i8 - from.0 as i8).abs();
    let dy = (to.1 as i8 - from.1 as i8).abs();
    (dx == 2 && dy == 1) || (dx == 1 && dy == 2)
}

fn is_valid_bishop_move(from: (u8, u8), to: (u8, u8), board_state: &BoardState) -> bool {
    let dx = (to.0 as i8 - from.0 as i8).abs();
    let dy = (to.1 as i8 - from.1 as i8).abs();

    // Must move diagonally
    if dx != dy {
        return false;
    }

    // Check path is clear
    is_path_clear(from, to, board_state)
}

fn is_valid_rook_move(from: (u8, u8), to: (u8, u8), board_state: &BoardState) -> bool {
    // Must move horizontally or vertically
    if from.0 != to.0 && from.1 != to.1 {
        return false;
    }

    // Check path is clear
    is_path_clear(from, to, board_state)
}

fn is_valid_queen_move(from: (u8, u8), to: (u8, u8), board_state: &BoardState) -> bool {
    // Queen moves like rook or bishop
    is_valid_rook_move(from, to, board_state) || is_valid_bishop_move(from, to, board_state)
}

fn is_valid_king_move(from: (u8, u8), to: (u8, u8), _has_moved: bool) -> bool {
    let dx = (to.0 as i8 - from.0 as i8).abs();
    let dy = (to.1 as i8 - from.1 as i8).abs();

    // King moves one square in any direction
    dx <= 1 && dy <= 1

    // TODO: Castling
}

fn is_path_clear(from: (u8, u8), to: (u8, u8), board_state: &BoardState) -> bool {
    let dx = (to.0 as i8 - from.0 as i8).signum();
    let dy = (to.1 as i8 - from.1 as i8).signum();

    let mut x = from.0 as i8 + dx;
    let mut y = from.1 as i8 + dy;

    while (x, y) != (to.0 as i8, to.1 as i8) {
        if !board_state.is_empty((x as u8, y as u8)) {
            return false;
        }
        x += dx;
        y += dy;
    }

    true
}
