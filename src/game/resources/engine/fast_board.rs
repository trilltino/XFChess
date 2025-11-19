//! Fast board state lookup using bitboards
//!
//! Provides O(1) square occupancy checks using bitwise operations instead of
//! iterating through all pieces. Critical for move validation performance.
//!
//! # Bitboard Representation
//!
//! Each bitboard is a u64 where each bit represents a square on the 8x8 board:
//! - Bit 0 = a1 (rank 0, file 0)
//! - Bit 7 = h1 (rank 0, file 7)
//! - Bit 56 = a8 (rank 7, file 0)
//! - Bit 63 = h8 (rank 7, file 7)
//!
//! # Performance Benefits
//!
//! **Before** (O(n) iteration):
//! ```ignore
//! for (entity, piece, _, _) in pieces.iter() {
//!     if piece.x == target_x && piece.y == target_y {
//!         return Some(entity);
//!     }
//! }
//! ```
//!
//! **After** (O(1) bitwise check):
//! ```ignore
//! let index = y * 8 + x;
//! board.occupied & (1u64 << index) != 0
//! ```
//!
//! # Reference
//!
//! - `crates/chess_engine/src/bitset.rs` - BitSet implementation pattern
//! - Chess programming wiki: https://www.chessprogramming.org/Bitboards

use bevy::prelude::*;

/// Fast board state using bitboards for O(1) piece lookups
///
/// Updated by systems after each move to maintain sync with ECS piece positions.
/// Used by move validation to quickly check square occupancy.
#[derive(Resource, Default, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct FastBoardState {
    /// All occupied squares (white | black)
    pub occupied: u64,

    /// White piece positions
    pub white_pieces: u64,

    /// Black piece positions
    pub black_pieces: u64,

    /// Tracks if board state is dirty and needs rebuild
    pub dirty: bool,
}

impl FastBoardState {
    /// Create empty board
    #[allow(dead_code)] // Public API - useful for testing and initialization
    pub fn new() -> Self {
        Self {
            occupied: 0,
            white_pieces: 0,
            black_pieces: 0,
            dirty: true,
        }
    }

    /// Convert board coordinates to bit index (0-63)
    ///
    /// # Arguments
    /// * `x` - Rank (0-7, where 0 is rank 1)
    /// * `y` - File (0-7, where 0 is file a)
    ///
    /// # Returns
    /// Bit index in range 0..64
    #[inline]
    pub fn square_to_index(x: u8, y: u8) -> usize {
        (x as usize) * 8 + (y as usize)
    }

    /// Check if a square is occupied by any piece
    ///
    /// # Arguments
    /// * `x` - Rank (0-7)
    /// * `y` - File (0-7)
    ///
    /// # Returns
    /// `true` if square contains a piece, `false` if empty
    #[inline]
    #[allow(dead_code)] // Public API - useful for move validation and debugging
    pub fn is_occupied(&self, x: u8, y: u8) -> bool {
        let index = Self::square_to_index(x, y);
        (self.occupied & (1u64 << index)) != 0
    }

    /// Check if a square is occupied by a white piece
    #[inline]
    #[allow(dead_code)] // Public API - useful for move validation and debugging
    pub fn is_white(&self, x: u8, y: u8) -> bool {
        let index = Self::square_to_index(x, y);
        (self.white_pieces & (1u64 << index)) != 0
    }

    /// Check if a square is occupied by a black piece
    #[inline]
    #[allow(dead_code)] // Public API - useful for move validation and debugging
    pub fn is_black(&self, x: u8, y: u8) -> bool {
        let index = Self::square_to_index(x, y);
        (self.black_pieces & (1u64 << index)) != 0
    }

    /// Set a square as occupied by a white piece
    #[inline]
    pub fn set_white(&mut self, x: u8, y: u8) {
        let index = Self::square_to_index(x, y);
        let mask = 1u64 << index;
        self.white_pieces |= mask;
        self.occupied |= mask;
    }

    /// Set a square as occupied by a black piece
    #[inline]
    pub fn set_black(&mut self, x: u8, y: u8) {
        let index = Self::square_to_index(x, y);
        let mask = 1u64 << index;
        self.black_pieces |= mask;
        self.occupied |= mask;
    }

    /// Clear a square (remove any piece)
    #[inline]
    #[allow(dead_code)] // Public API - useful for move execution and testing
    pub fn clear_square(&mut self, x: u8, y: u8) {
        let index = Self::square_to_index(x, y);
        let mask = !(1u64 << index);
        self.white_pieces &= mask;
        self.black_pieces &= mask;
        self.occupied &= mask;
    }

    /// Move a piece from one square to another
    ///
    /// Handles captures automatically by clearing the destination square first.
    #[inline]
    #[allow(dead_code)] // Public API - useful for move execution and testing
    pub fn move_piece(&mut self, from_x: u8, from_y: u8, to_x: u8, to_y: u8) {
        let is_white = self.is_white(from_x, from_y);

        // Clear both squares
        self.clear_square(from_x, from_y);
        self.clear_square(to_x, to_y);

        // Set destination
        if is_white {
            self.set_white(to_x, to_y);
        } else {
            self.set_black(to_x, to_y);
        }
    }

    /// Clear all pieces from the board
    #[inline]
    pub fn clear(&mut self) {
        self.occupied = 0;
        self.white_pieces = 0;
        self.black_pieces = 0;
    }

    /// Mark board as dirty (needs rebuild from ECS)
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Count total pieces on board
    #[inline]
    pub fn piece_count(&self) -> u32 {
        self.occupied.count_ones()
    }

    /// Count white pieces
    #[inline]
    pub fn white_count(&self) -> u32 {
        self.white_pieces.count_ones()
    }

    /// Count black pieces
    #[inline]
    pub fn black_count(&self) -> u32 {
        self.black_pieces.count_ones()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_square_to_index() {
        assert_eq!(FastBoardState::square_to_index(0, 0), 0); // a1
        assert_eq!(FastBoardState::square_to_index(0, 7), 7); // h1
        assert_eq!(FastBoardState::square_to_index(7, 0), 56); // a8
        assert_eq!(FastBoardState::square_to_index(7, 7), 63); // h8
        assert_eq!(FastBoardState::square_to_index(3, 4), 28); // e4
    }

    #[test]
    fn test_set_and_check_white() {
        let mut board = FastBoardState::new();
        board.set_white(0, 0);

        assert!(board.is_occupied(0, 0));
        assert!(board.is_white(0, 0));
        assert!(!board.is_black(0, 0));
        assert_eq!(board.piece_count(), 1);
        assert_eq!(board.white_count(), 1);
        assert_eq!(board.black_count(), 0);
    }

    #[test]
    fn test_set_and_check_black() {
        let mut board = FastBoardState::new();
        board.set_black(7, 7);

        assert!(board.is_occupied(7, 7));
        assert!(board.is_black(7, 7));
        assert!(!board.is_white(7, 7));
        assert_eq!(board.piece_count(), 1);
        assert_eq!(board.white_count(), 0);
        assert_eq!(board.black_count(), 1);
    }

    #[test]
    fn test_clear_square() {
        let mut board = FastBoardState::new();
        board.set_white(3, 4);
        assert!(board.is_occupied(3, 4));

        board.clear_square(3, 4);
        assert!(!board.is_occupied(3, 4));
        assert_eq!(board.piece_count(), 0);
    }

    #[test]
    fn test_move_piece() {
        let mut board = FastBoardState::new();
        board.set_white(0, 0);

        board.move_piece(0, 0, 3, 4);

        assert!(!board.is_occupied(0, 0));
        assert!(board.is_occupied(3, 4));
        assert!(board.is_white(3, 4));
        assert_eq!(board.piece_count(), 1);
    }

    #[test]
    fn test_move_piece_with_capture() {
        let mut board = FastBoardState::new();
        board.set_white(0, 0);
        board.set_black(3, 4);
        assert_eq!(board.piece_count(), 2);

        board.move_piece(0, 0, 3, 4); // White captures black

        assert!(!board.is_occupied(0, 0));
        assert!(board.is_occupied(3, 4));
        assert!(board.is_white(3, 4));
        assert!(!board.is_black(3, 4));
        assert_eq!(board.piece_count(), 1);
    }

    #[test]
    fn test_multiple_pieces() {
        let mut board = FastBoardState::new();

        // Set up a starting position pattern
        for file in 0..8 {
            board.set_white(1, file); // White pawns on rank 2
            board.set_black(6, file); // Black pawns on rank 7
        }

        assert_eq!(board.piece_count(), 16);
        assert_eq!(board.white_count(), 8);
        assert_eq!(board.black_count(), 8);

        // Check specific squares
        assert!(board.is_white(1, 0));
        assert!(board.is_black(6, 7));
        assert!(!board.is_occupied(4, 4)); // Empty center
    }

    #[test]
    fn test_clear_board() {
        let mut board = FastBoardState::new();
        board.set_white(0, 0);
        board.set_black(7, 7);
        assert_eq!(board.piece_count(), 2);

        board.clear();

        assert_eq!(board.piece_count(), 0);
        assert!(!board.is_occupied(0, 0));
        assert!(!board.is_occupied(7, 7));
    }
}
