//! Board utilities and helper functions
//!
//! Provides fundamental board operations used throughout the engine:
//! - Square validation and indexing
//! - Piece queries
//! - Board boundaries checking

use super::constants::*;
use super::types::*;

/// Convert column and row to linear position (0-63)
#[inline]
pub fn square_to_pos(col: i8, row: i8) -> i8 {
    row * 8 + col
}

/// Convert position to (col, row)
#[inline]
pub fn pos_to_square(pos: i8) -> (i8, i8) {
    (pos % 8, pos / 8)
}

/// Check if position is within board bounds
#[inline]
pub fn is_valid_pos(pos: i32) -> bool {
    pos >= 0 && pos < 64
}

/// Check if square coordinates are valid
#[inline]
#[allow(dead_code)] // Part of engine's public API - utility function
pub fn is_valid_square(col: i8, row: i8) -> bool {
    col >= 0 && col < 8 && row >= 0 && row < 8
}

/// Get piece at position
#[inline]
#[allow(dead_code)] // Part of engine's public API - utility function
pub fn get_piece_at(board: &Board, pos: i8) -> i8 {
    board[pos as usize]
}

/// Check if square is empty
#[inline]
#[allow(dead_code)] // Part of engine's public API - utility function
pub fn is_empty(board: &Board, pos: i8) -> bool {
    board[pos as usize] == 0
}

/// Check if piece belongs to color (1 = white, -1 = black)
#[inline]
pub fn piece_belongs_to(piece: i8, color: Color) -> bool {
    if piece == 0 {
        false
    } else if color > 0 {
        piece > 0
    } else {
        piece < 0
    }
}

/// Get color of piece (1 = white, -1 = black, 0 = empty)
#[inline]
#[allow(dead_code)] // Part of engine's public API - utility function
pub fn get_piece_color(piece: i8) -> Color {
    if piece > 0 {
        COLOR_WHITE
    } else if piece < 0 {
        COLOR_BLACK
    } else {
        0
    }
}

/// Check if moving from src to dst crosses file boundary (for knights/kings)
#[allow(dead_code)] // Part of engine's public API - utility function
pub fn crosses_file_boundary(src: i32, dst: i32, _direction: i32) -> bool {
    let src_file = src % 8;
    let dst_file = dst % 8;
    (src_file - dst_file).abs() > 2
}

/// Initialize a game board to standard starting position
pub fn init_board() -> Board {
    SETUP
}
