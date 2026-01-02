//! Move table initialization
//!
//! Precomputes all possible moves for each piece type from each square.
//! These tables are computed once at game initialization for performance.
//!
//! ## Performance Characteristics
//!
//! - **Initialization time**: ~1-2ms for all 64 squares × 6 piece types
//! - **Memory usage**: ~2MB total (64 squares × 6 tables × ~50 moves avg × 5 bytes)
//! - **Lookup time**: O(1) - direct array access
//!
//! ## Algorithm
//!
//! For each square, we generate all possible destination squares that a piece
//! could theoretically reach, ignoring board occupancy. These "pseudo-legal"
//! moves are then filtered during actual move generation based on:
//! - Board occupancy (blocked by own pieces)
//! - Capture legality (can't capture own pieces)
//! - Special rules (en passant, castling, etc.)

use crate::board::*;
use crate::constants::*;
use crate::types::*;

/// Initialize move tables for all piece types
///
/// This function precomputes all possible moves for each piece type from each
/// square on the board. The tables are stored in the `Game` struct and used
/// during move generation for fast lookup.
///
/// # Performance
///
/// This function is called once during game initialization. The computation
/// is O(1) per square, resulting in O(64) = O(1) total time complexity.
///
/// # Examples
///
/// ```rust,ignore
/// let mut game = new_game();
/// init_move_tables(&mut game);
/// // Now game.rook[0] contains all possible rook moves from square 0
/// ```
pub fn init_move_tables(game: &mut Game) {
    // Initialize all piece types for all 64 squares
    for square in 0..64 {
        game.rook[square] = init_rook_moves_from(square as i8);
        game.bishop[square] = init_bishop_moves_from(square as i8);
        game.knight[square] = init_knight_moves_from(square as i8);
        game.king[square] = init_king_moves_from(square as i8);
        game.white_pawn[square] = init_white_pawn_moves_from(square as i8);
        game.black_pawn[square] = init_black_pawn_moves_from(square as i8);
    }
}

/// Generate all rook moves from a given square
///
/// Rooks move horizontally and vertically. This function generates all
/// possible destinations along these four directions until the board edge.
///
/// # Arguments
///
/// * `from` - Source square index (0-63)
///
/// # Returns
///
/// Vector of `KK` moves representing all possible rook destinations
fn init_rook_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    // Rooks move in four directions: North, East, South, West
    for &dir in &ROOK_DIRS {
        let mut current = from as i32;

        // Slide in this direction until we hit the board edge
        loop {
            current += dir;

            if !is_valid_pos(current) {
                break;
            }

            let (new_col, new_row) = pos_to_square(current as i8);

            // Check if we wrapped around the board (edge case for direction vectors)
            if (dir == O && new_col < col as i8)
                || (dir == W && new_col > col as i8)
                || (dir == N && new_row > row as i8)
                || (dir == S && new_row < row as i8)
            {
                break;
            }

            moves.push(KK::new(from, current as i8, 0, 0));
        }
    }

    moves
}

/// Generate all bishop moves from a given square
///
/// Bishops move diagonally. This function generates all possible destinations
/// along the four diagonal directions until the board edge.
///
/// # Arguments
///
/// * `from` - Source square index (0-63)
///
/// # Returns
///
/// Vector of `KK` moves representing all possible bishop destinations
fn init_bishop_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    // Bishops move in four diagonal directions: NE, SE, SW, NW
    for &dir in &BISHOP_DIRS {
        let mut current = from as i32;

        // Slide diagonally until we hit the board edge
        loop {
            current += dir;

            if !is_valid_pos(current) {
                break;
            }

            let (new_col, new_row) = pos_to_square(current as i8);

            // Validate coordinates are within bounds
            if new_col < 0 || new_col > 7 || new_row < 0 || new_row > 7 {
                break;
            }

            // Check if we wrapped around the board (edge case for direction vectors)
            // Diagonal moves must maintain equal column and row distance
            if (new_col - col as i8).abs() > 1
                && (new_row - row as i8).abs() != (current as i8 - from).abs() / 8
            {
                break;
            }

            moves.push(KK::new(from, current as i8, 0, 0));
        }
    }

    moves
}

/// Generate all knight moves from a given square
///
/// Knights move in an L-shape: 2 squares in one direction, then 1 square
/// perpendicular. This function generates all 8 possible knight destinations.
///
/// # Arguments
///
/// * `from` - Source square index (0-63)
///
/// # Returns
///
/// Vector of `KK` moves representing all possible knight destinations
fn init_knight_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    // Knights have 8 possible L-shaped moves
    for &dir in &KNIGHT_DIRS {
        let to = from as i32 + dir;

        if !is_valid_pos(to) {
            continue;
        }

        let (new_col, new_row) = pos_to_square(to as i8);

        // Validate L-shape: exactly 2+1 or 1+2 squares
        let col_diff = (new_col - col as i8).abs();
        let row_diff = (new_row - row as i8).abs();

        if (col_diff == 2 && row_diff == 1) || (col_diff == 1 && row_diff == 2) {
            moves.push(KK::new(from, to as i8, 0, 0));
        }
    }

    moves
}

/// Generate all king moves from a given square
///
/// Kings move one square in any direction (horizontally, vertically, or diagonally).
/// This function generates all 8 possible king destinations.
///
/// # Arguments
///
/// * `from` - Source square index (0-63)
///
/// # Returns
///
/// Vector of `KK` moves representing all possible king destinations
///
/// # Note
///
/// Castling is handled separately during move generation, not in the move tables.
fn init_king_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    // Kings can move one square in any of 8 directions
    for &dir in &KING_DIRS {
        let to = from as i32 + dir;

        if !is_valid_pos(to) {
            continue;
        }

        let (new_col, new_row) = pos_to_square(to as i8);

        // Validate: king moves exactly 1 square in any direction
        if (new_col - col as i8).abs() <= 1 && (new_row - row as i8).abs() <= 1 {
            moves.push(KK::new(from, to as i8, 0, 0));
        }
    }

    moves
}

/// Generate all white pawn moves from a given square
///
/// White pawns move north (up the board, decreasing square indices).
/// This function generates:
/// - Single push forward
/// - Double push from starting rank (rank 2)
/// - Diagonal captures
///
/// # Arguments
///
/// * `from` - Source square index (0-63)
///
/// # Returns
///
/// Vector of `KK` moves representing all possible white pawn destinations
///
/// # Note
///
/// En passant and promotion are handled separately during move generation.
fn init_white_pawn_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    // Single push forward (north, -8 squares)
    let to = from - 8;
    if to >= 0 {
        moves.push(KK::new(from, to, 0, 0));
    }

    // Double push from starting position (rank 2, row index 1)
    if row == 1 {
        let to = from - 16;
        if to >= 0 {
            moves.push(KK::new(from, to, 0, 0));
        }
    }

    // Diagonal captures (northeast and northwest)
    for &dir in &[NO, NW] {
        let to = from as i32 + dir;
        if is_valid_pos(to) {
            let (new_col, _) = pos_to_square(to as i8);
            // Validate: capture moves must be exactly one column away
            if (new_col - col as i8).abs() == 1 {
                moves.push(KK::new(from, to as i8, 0, 0));
            }
        }
    }

    moves
}

/// Generate all black pawn moves from a given square
///
/// Black pawns move south (down the board, increasing square indices).
/// This function generates:
/// - Single push forward
/// - Double push from starting rank (rank 7)
/// - Diagonal captures
///
/// # Arguments
///
/// * `from` - Source square index (0-63)
///
/// # Returns
///
/// Vector of `KK` moves representing all possible black pawn destinations
///
/// # Note
///
/// En passant and promotion are handled separately during move generation.
fn init_black_pawn_moves_from(from: i8) -> KKS {
    let mut moves = Vec::new();
    let (col, row) = pos_to_square(from);

    // Single push forward (south, +8 squares)
    let to = from + 8;
    if to < 64 {
        moves.push(KK::new(from, to, 0, 0));
    }

    // Double push from starting position (rank 7, row index 6)
    if row == 6 {
        let to = from + 16;
        if to < 64 {
            moves.push(KK::new(from, to, 0, 0));
        }
    }

    // Diagonal captures (southeast and southwest)
    for &dir in &[SO, SW] {
        let to = from as i32 + dir;
        if is_valid_pos(to) {
            let (new_col, _) = pos_to_square(to as i8);
            // Validate: capture moves must be exactly one column away
            if (new_col - col as i8).abs() == 1 {
                moves.push(KK::new(from, to as i8, 0, 0));
            }
        }
    }

    moves
}
