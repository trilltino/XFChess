//! # BitSet - Efficient Board Representation Using Bitboards
//!
//! ## Overview
//!
//! This module implements a BitSet data structure based on the **bitboard** technique, a fundamental
//! optimization in computer chess that represents board positions using single 64-bit integers. Each
//! bit in the integer corresponds to one square on the chess board, enabling extremely fast set
//! operations using native CPU bitwise instructions.
//!
//! ## Algorithm & Performance
//!
//! **Bitboards** (also called bitsets or bitmaps) were first described for chess by Georgy Adelson-Velsky
//! et al. in 1967 and have become the standard for modern chess engines due to their performance advantages:
//!
//! - **Single-cycle operations**: Most queries (checking if a square is occupied, counting pieces) execute
//!   in a single CPU cycle using hardware instructions like POPCNT (population count).
//! - **Parallel processing**: A single bitwise operation can query or modify up to 64 squares simultaneously.
//! - **Register-fit**: On 64-bit architectures, an entire bitboard fits in a single CPU register, enabling
//!   the fastest possible memory access.
//! - **Cache-friendly**: Compact representation (8 bytes vs 64 bytes for array-based boards) improves cache hit rates.
//!
//! Performance studies show bitboards are **2-5x faster** than array-based representations for move generation
//! in chess engines on modern 64-bit processors.
//!
//! ## Technical Details
//!
//! The chess board is mapped to bits 0-63 where:
//! - Bit 0 = A1 (bottom-left for white)
//! - Bit 7 = H1 (bottom-right for white)
//! - Bit 56 = A8 (top-left for white)
//! - Bit 63 = H8 (top-right for white)
//!
//! Common operations:
//! - **Insert**: `bitboard |= (1 << square)` - Sets bit at square (O(1))
//! - **Remove**: `bitboard &= !(1 << square)` - Clears bit at square (O(1))
//! - **Contains**: `(bitboard & (1 << square)) != 0` - Tests if square occupied (O(1))
//! - **Count**: `bitboard.count_ones()` - Hardware POPCNT instruction (O(1))
//!
//! ## XFChess Integration Strategy
//!
//! ### Current XFChess Board Representation
//!
//! XFChess currently uses an **ECS-based representation** where each piece is an entity with a `Piece` component
//! containing `(x: u8, y: u8)` coordinates. While this is clean and idiomatic for Bevy, it requires:
//! - Iterating over all piece entities to check square occupancy (O(n) where n = number of pieces)
//! - Multiple query operations to find pieces for move validation
//! - No way to check multiple squares simultaneously
//!
//! ### Integration Approach: Hybrid Model
//!
//! **Recommended**: Keep ECS for rendering/game logic, add BitSet for move validation:
//!
//! ```rust,ignore
//! // Add to game/resources/board_state.rs
//! #[derive(Resource)]
//! pub struct FastBoardState {
//!     white_pieces: BitSet,  // All white piece positions
//!     black_pieces: BitSet,  // All black piece positions
//!     all_pieces: BitSet,    // Union of both colors
//! }
//!
//! impl FastBoardState {
//!     /// Rebuild from ECS queries (called after each move)
//!     pub fn sync_from_ecs(&mut self, pieces: &Query<&Piece>) {
//!         self.white_pieces.clear();
//!         self.black_pieces.clear();
//!
//!         for piece in pieces.iter() {
//!             let square = piece.y * 8 + piece.x;
//!             match piece.color {
//!                 PieceColor::White => self.white_pieces.insert(square),
//!                 PieceColor::Black => self.black_pieces.insert(square),
//!             }
//!         }
//!
//!         self.all_pieces.0 = self.white_pieces.0 | self.black_pieces.0;
//!     }
//!
//!     /// Fast square occupancy check (O(1) vs O(n) ECS iteration)
//!     pub fn is_square_occupied(&self, x: u8, y: u8) -> bool {
//!         let square = y * 8 + x;
//!         self.all_pieces.contains(square)
//!     }
//!
//!     /// Count pieces (instant with POPCNT)
//!     pub fn piece_count(&self) -> u32 {
//!         self.all_pieces.count_ones()
//!     }
//! }
//! ```
//!
//! ### Benefits for XFChess
//!
//! 1. **Faster move validation**: Checking if a rook path is clear becomes `!(bitboard & path_mask).count_ones() == 0`
//!    instead of iterating through all pieces.
//! 2. **Efficient legal move generation**: Can quickly compute all attacked squares by ORing piece attack patterns.
//! 3. **AI integration**: The chess engine's move generation requires bitboard representation - this provides the bridge.
//! 4. **Endgame detection**: Instantly check piece counts without iteration.
//!
//! ### Migration Path
//!
//! **Phase 1** (No breaking changes):
//! - Add `FastBoardState` resource alongside existing ECS
//! - Sync after each move in `movement.rs`
//! - Use for move validation in `piece_moves.rs`
//!
//! **Phase 2** (Optional optimization):
//! - Generate legal moves using bitboard techniques
//! - Integrate chess engine AI using shared BitSet representation
//!
//! ## Further Reading
//!
//! - **Chess Programming Wiki - Bitboards**: https://www.chessprogramming.org/Bitboards
//! - **Wikipedia - Bitboard**: https://en.wikipedia.org/wiki/Bitboard
//! - **Board Representation (Computer Chess)**: https://en.wikipedia.org/wiki/Board_representation_(computer_chess)
//! - **Stack Overflow - Chess Bitboards**: https://stackoverflow.com/questions/39874/how-do-i-model-a-chessboard-when-programming-a-computer-to-play-chess
//!
//! ## Historical Note
//!
//! The bitboard method was invented by Christopher Strachey in 1952 for his checkers program, making it
//! one of the oldest computer game programming techniques still in active use. Its longevity speaks to
//! the fundamental efficiency of using CPU-native operations for game state representation.

#[derive(Copy, Clone, Debug, Default)]
pub struct BitSet(pub u64);

impl BitSet {
    pub fn new() -> Self {
        BitSet(0)
    }

    pub fn insert<T>(&mut self, index: T)
    where
        u64: std::ops::Shl<T, Output = u64>,
    {
        self.0 |= 1 << index;
    }

    pub fn remove<T>(&mut self, index: T)
    where
        u64: std::ops::Shl<T, Output = u64>,
    {
        self.0 &= !(1 << index);
    }

    pub fn contains<T>(&self, index: T) -> bool
    where
        u64: std::ops::Shl<T, Output = u64>,
    {
        (self.0 & (1 << index)) != 0
    }

    #[inline]
    pub fn count_ones(&self) -> u32 {
        self.0.count_ones()
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }
}
