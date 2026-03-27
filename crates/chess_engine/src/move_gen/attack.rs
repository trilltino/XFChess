//! Attack detection and square checking
//!
//! Provides functions to check if squares are under attack and if kings are in check.
//! This module is critical for move legality validation and check detection.
//!
//! ## Performance
//!
//! Attack detection is called frequently during move generation and search.
//! The functions use precomputed move tables where possible for O(1) lookups.
//!
//! ## Algorithm
//!
//! To check if a square is attacked, we iterate through all opponent pieces
//! and verify if any can reach the target square. This is more efficient than
//! generating all moves and checking if any target the square.

use crate::board::*;
use crate::constants::*;
use crate::types::*;

/// Check if a square is under attack by pieces of the specified color
///
/// This function determines if any piece of `by_color` can attack the target
/// square. It's used for:
/// - Check detection (is the king attacked?)
/// - Move legality (does this move leave the king in check?)
/// - Square safety evaluation
///
/// # Arguments
///
/// * `game` - The current game state
/// * `square` - Target square index (0-63) to check
/// * `by_color` - Color of pieces that might attack (1 for White, -1 for Black)
///
/// # Returns
///
/// `true` if the square is attacked by any piece of the specified color
///
/// # Examples
///
/// ```rust,ignore
/// // Check if square e4 is attacked by black pieces
/// let attacked = is_square_attacked(&game, 28, COLOR_BLACK);
/// ```
pub fn is_square_attacked(game: &Game, square: i8, by_color: Color) -> bool {
    // Check all pieces of the attacking color
    for from in 0..64 {
        let piece = game.board[from];

        if !piece_belongs_to(piece, by_color) {
            continue;
        }

        let piece_type = piece.abs();
        let can_attack = match piece_type {
            PAWN_ID => can_pawn_attack(game, from as i8, square, by_color),
            KNIGHT_ID => can_knight_attack(game, from as i8, square),
            BISHOP_ID => can_bishop_attack(game, from as i8, square),
            ROOK_ID => can_rook_attack(game, from as i8, square),
            QUEEN_ID => can_queen_attack(game, from as i8, square),
            KING_ID => can_king_attack(game, from as i8, square),
            _ => false,
        };

        if can_attack {
            return true;
        }
    }

    false
}

/// Check if a pawn can attack a target square
///
/// Pawns attack diagonally forward. This function uses the precomputed
/// pawn move tables to check if the target square is in the pawn's attack pattern.
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square of the pawn
/// * `to` - Target square to check
/// * `color` - Color of the pawn (1 for White, -1 for Black)
///
/// # Returns
///
/// `true` if the pawn can attack the target square
fn can_pawn_attack(game: &Game, from: i8, to: i8, color: Color) -> bool {
    // Use precomputed pawn move table
    let candidates = if color > 0 {
        &game.white_pawn[from as usize]
    } else {
        &game.black_pawn[from as usize]
    };

    let (from_col, _) = pos_to_square(from);
    let (to_col, _) = pos_to_square(to);

    // Pawns attack diagonally, so the column must differ by exactly 1
    candidates
        .iter()
        .any(|m| m.dst == to && (to_col - from_col).abs() == 1)
}

/// Check if a knight can attack a target square
///
/// Knights move in an L-shape (2+1 or 1+2 squares). This function validates
/// the L-shape pattern without checking board occupancy (knights jump over pieces).
///
/// # Arguments
///
/// * `_game` - The current game state (unused, knights don't need board state)
/// * `from` - Source square of the knight
/// * `to` - Target square to check
///
/// # Returns
///
/// `true` if the knight can attack the target square
fn can_knight_attack(_game: &Game, from: i8, to: i8) -> bool {
    let (from_col, from_row) = pos_to_square(from);
    let (to_col, to_row) = pos_to_square(to);
    let col_diff = (to_col - from_col).abs();
    let row_diff = (to_row - from_row).abs();

    // Valid L-shape: exactly 2+1 or 1+2 squares
    (col_diff == 2 && row_diff == 1) || (col_diff == 1 && row_diff == 2)
}

/// Check if a bishop can attack a target square
///
/// Bishops move diagonally. This function checks:
/// 1. The target is on the same diagonal
/// 2. No pieces block the path
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square of the bishop
/// * `to` - Target square to check
///
/// # Returns
///
/// `true` if the bishop can attack the target square (unblocked diagonal path)
fn can_bishop_attack(game: &Game, from: i8, to: i8) -> bool {
    let (from_col, from_row) = pos_to_square(from);
    let (to_col, to_row) = pos_to_square(to);

    // Must be on same diagonal (equal column and row distance)
    if (to_col - from_col).abs() != (to_row - from_row).abs() {
        return false;
    }

    // Determine direction and check for blocking pieces
    let col_dir = (to_col - from_col).signum();
    let row_dir = (to_row - from_row).signum();

    let mut current_col = from_col + col_dir;
    let mut current_row = from_row + row_dir;

    // Check each square along the diagonal path
    while current_col != to_col {
        let pos = square_to_pos(current_col, current_row);
        if game.board[pos as usize] != 0 {
            return false; // Path is blocked
        }
        current_col += col_dir;
        current_row += row_dir;
    }

    true
}

/// Check if a rook can attack a target square
///
/// Rooks move horizontally and vertically. This function checks:
/// 1. The target is on the same rank or file
/// 2. No pieces block the path
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square of the rook
/// * `to` - Target square to check
///
/// # Returns
///
/// `true` if the rook can attack the target square (unblocked horizontal/vertical path)
fn can_rook_attack(game: &Game, from: i8, to: i8) -> bool {
    let (from_col, from_row) = pos_to_square(from);
    let (to_col, to_row) = pos_to_square(to);

    // Must be on same rank or file
    if from_col != to_col && from_row != to_row {
        return false;
    }

    // Check horizontal path (same rank)
    if from_col == to_col {
        let dir = (to_row - from_row).signum();
        let mut current = from_row + dir;

        while current != to_row {
            let pos = square_to_pos(from_col, current);
            if game.board[pos as usize] != 0 {
                return false; // Path is blocked
            }
            current += dir;
        }
    } else {
        // Check vertical path (same file)
        let dir = (to_col - from_col).signum();
        let mut current = from_col + dir;

        while current != to_col {
            let pos = square_to_pos(current, from_row);
            if game.board[pos as usize] != 0 {
                return false; // Path is blocked
            }
            current += dir;
        }
    }

    true
}

/// Check if a queen can attack a target square
///
/// Queens combine the movement of rooks and bishops. This function checks
/// if the target is reachable via either horizontal/vertical or diagonal movement.
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square of the queen
/// * `to` - Target square to check
///
/// # Returns
///
/// `true` if the queen can attack the target square
fn can_queen_attack(game: &Game, from: i8, to: i8) -> bool {
    // Queen moves like both rook and bishop
    can_rook_attack(game, from, to) || can_bishop_attack(game, from, to)
}

/// Check if a king can attack a target square
///
/// Kings move one square in any direction. This function checks if the target
/// is exactly one square away (horizontally, vertically, or diagonally).
///
/// # Arguments
///
/// * `_game` - The current game state (unused, kings don't need board state for range check)
/// * `from` - Source square of the king
/// * `to` - Target square to check
///
/// # Returns
///
/// `true` if the king can attack the target square (within one square)
fn can_king_attack(_game: &Game, from: i8, to: i8) -> bool {
    let (from_col, from_row) = pos_to_square(from);
    let (to_col, to_row) = pos_to_square(to);

    // King moves exactly one square in any direction
    (to_col - from_col).abs() <= 1 && (to_row - from_row).abs() <= 1
}

/// Find the king position for a given color
///
/// Searches the board for the king of the specified color. This is used
/// for check detection and king safety evaluation.
///
/// # Arguments
///
/// * `game` - The current game state
/// * `color` - Color of the king to find (1 for White, -1 for Black)
///
/// # Returns
///
/// `Some(square)` if the king is found, `None` if not found (should never happen in valid positions)
///
/// # Examples
///
/// ```rust,ignore
/// if let Some(king_pos) = find_king(&game, COLOR_WHITE) {
///     println!("White king is at square {}", king_pos);
/// }
/// ```
pub fn find_king(game: &Game, color: Color) -> Option<i8> {
    let king_piece = if color > 0 { W_KING } else { B_KING };

    for square in 0..64 {
        if game.board[square] == king_piece {
            return Some(square as i8);
        }
    }

    None
}

/// Check if the king of a given color is in check
///
/// This is a convenience function that combines `find_king` and `is_square_attacked`.
/// It's the most common check operation during move generation and search.
///
/// # Arguments
///
/// * `game` - The current game state
/// * `color` - Color of the king to check (1 for White, -1 for Black)
///
/// # Returns
///
/// `true` if the king is in check (under attack by opponent pieces)
///
/// # Examples
///
/// ```rust,ignore
/// if is_in_check(&game, COLOR_WHITE) {
///     println!("White is in check!");
/// }
/// ```
pub fn is_in_check(game: &Game, color: Color) -> bool {
    if let Some(king_pos) = find_king(game, color) {
        is_square_attacked(game, king_pos, -color)
    } else {
        false
    }
}
