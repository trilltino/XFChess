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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{
        BISHOP_ID, B_BISHOP, B_KING, B_KNIGHT, B_PAWN, B_QUEEN, B_ROOK, COLOR_BLACK, COLOR_WHITE,
        KING_ID, KNIGHT_ID, PAWN_ID, QUEEN_ID, ROOK_ID, W_BISHOP, W_KING, W_KNIGHT, W_PAWN,
        W_QUEEN, W_ROOK,
    };

    #[test]
    fn test_square_to_pos() {
        // a1 = (0, 0) -> 0
        assert_eq!(square_to_pos(0, 0), 0);
        // h1 = (7, 0) -> 7
        assert_eq!(square_to_pos(7, 0), 7);
        // a8 = (0, 7) -> 56
        assert_eq!(square_to_pos(0, 7), 56);
        // e4 = (4, 3) -> 28
        assert_eq!(square_to_pos(4, 3), 28);
        // h8 = (7, 7) -> 63
        assert_eq!(square_to_pos(7, 7), 63);
    }

    #[test]
    fn test_pos_to_square() {
        assert_eq!(pos_to_square(0), (0, 0)); // a1
        assert_eq!(pos_to_square(7), (7, 0)); // h1
        assert_eq!(pos_to_square(56), (0, 7)); // a8
        assert_eq!(pos_to_square(28), (4, 3)); // e4
        assert_eq!(pos_to_square(63), (7, 7)); // h8
    }

    #[test]
    fn test_square_conversion_round_trip() {
        for pos in 0..64 {
            let (col, row) = pos_to_square(pos);
            assert_eq!(square_to_pos(col, row), pos);
        }
    }

    #[test]
    fn test_is_valid_pos() {
        // Valid positions
        assert!(is_valid_pos(0));
        assert!(is_valid_pos(32));
        assert!(is_valid_pos(63));

        // Invalid positions
        assert!(!is_valid_pos(-1));
        assert!(!is_valid_pos(64));
        assert!(!is_valid_pos(100));
    }

    #[test]
    fn test_is_valid_square() {
        // Valid squares
        assert!(is_valid_square(0, 0));
        assert!(is_valid_square(7, 7));
        assert!(is_valid_square(4, 3));

        // Invalid squares
        assert!(!is_valid_square(-1, 0));
        assert!(!is_valid_square(0, -1));
        assert!(!is_valid_square(8, 0));
        assert!(!is_valid_square(0, 8));
    }

    #[test]
    fn test_get_piece_at() {
        let board = init_board();

        // White pieces on first rank
        assert_eq!(get_piece_at(&board, 0), W_ROOK); // a1
        assert_eq!(get_piece_at(&board, 3), W_KING); // d1 (engine uses d1 for king)

        // Black pieces on eighth rank
        assert_eq!(get_piece_at(&board, 56), B_ROOK); // a8
        assert_eq!(get_piece_at(&board, 59), B_KING); // d8 (engine uses d8 for king)

        // Empty squares in the middle
        assert_eq!(get_piece_at(&board, 28), 0); // e4
    }

    #[test]
    fn test_is_empty() {
        let board = init_board();

        // Empty squares
        assert!(is_empty(&board, 28)); // e4
        assert!(is_empty(&board, 35)); // d5

        // Occupied squares
        assert!(!is_empty(&board, 0)); // a1 has rook
        assert!(!is_empty(&board, 12)); // e2 has pawn
    }

    #[test]
    fn test_piece_belongs_to() {
        // White pieces (positive)
        assert!(piece_belongs_to(PAWN_ID, COLOR_WHITE));
        assert!(piece_belongs_to(KING_ID, COLOR_WHITE));
        assert!(!piece_belongs_to(PAWN_ID, COLOR_BLACK));

        // Black pieces (negative)
        assert!(piece_belongs_to(-PAWN_ID, COLOR_BLACK));
        assert!(piece_belongs_to(-QUEEN_ID, COLOR_BLACK));
        assert!(!piece_belongs_to(-PAWN_ID, COLOR_WHITE));

        // Empty square belongs to neither
        assert!(!piece_belongs_to(0, COLOR_WHITE));
        assert!(!piece_belongs_to(0, COLOR_BLACK));
    }

    #[test]
    fn test_get_piece_color() {
        assert_eq!(get_piece_color(PAWN_ID), COLOR_WHITE);
        assert_eq!(get_piece_color(KING_ID), COLOR_WHITE);
        assert_eq!(get_piece_color(-PAWN_ID), COLOR_BLACK);
        assert_eq!(get_piece_color(-QUEEN_ID), COLOR_BLACK);
        assert_eq!(get_piece_color(0), 0);
    }

    #[test]
    fn test_init_board_starting_position() {
        let board = init_board();

        // Verify piece count
        let white_pieces: i32 = board.iter().filter(|&&p| p > 0).count() as i32;
        let black_pieces: i32 = board.iter().filter(|&&p| p < 0).count() as i32;
        assert_eq!(white_pieces, 16, "Should have 16 white pieces");
        assert_eq!(black_pieces, 16, "Should have 16 black pieces");

        // Verify king positions (engine uses d1/d8 for kings)
        assert_eq!(board[3], W_KING, "White king should be on d1");
        assert_eq!(board[59], B_KING, "Black king should be on d8");

        // Verify queens (engine uses e1/e8 for queens)
        assert_eq!(board[4], W_QUEEN, "White queen should be on e1");
        assert_eq!(board[60], B_QUEEN, "Black queen should be on e8");
    }
}
