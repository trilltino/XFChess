//! Board state representation for move validation
//!
//! Provides a lightweight snapshot of the current board state for pure function
//! move validation without coupling to ECS systems.
//!
//! # Architecture
//!
//! **Separation of Concerns**:
//! - **ECS World**: Authoritative source (entities, components, transforms)
//! - **BoardState**: Snapshot for validation (position queries, pure functions)
//!
//! Systems create a `BoardState` from ECS data, pass it to validation functions,
//! then apply validated moves back to the ECS world.
//!
//! # Performance vs Correctness Trade-offs
//!
//! BoardState uses a `Vec<(Entity, Piece, Position)>` for simplicity and
//! correctness. For better performance (2-5x faster), consider:
//! - [`crate::game::resources::FastBoardState`] - O(1) bitboard lookups
//! - `reference/chess_engine/bitset.rs` - Optimized bit manipulation
//!
//! Current implementation prioritizes:
//! - **Clarity**: Easy to understand and debug
//! - **Testability**: Simple to construct test scenarios
//! - **Correctness**: Matches ECS state exactly
//!
//! # Usage Pattern
//!
//! ```rust,ignore
//! // 1. Build board state from ECS world
//! let mut pieces_data = Vec::new();
//! for (entity, piece, transform) in pieces.iter() {
//!     let pos = world_to_board(transform.translation);
//!     pieces_data.push((entity, *piece, pos));
//! }
//! let board = BoardState { pieces: pieces_data };
//!
//! // 2. Validate moves using pure functions
//! let is_legal = is_valid_move(
//!     PieceType::Queen,
//!     PieceColor::White,
//!     from,
//!     to,
//!     &board,
//!     has_moved
//! );
//!
//! // 3. Apply validated move back to ECS
//! if is_legal {
//!     transform.translation = board_to_world(to);
//! }
//! ```
//!
//! # Future Enhancements
//!
//! - **Attack/Defense Maps**: Pre-compute all squares attacked by each color
//! - **Pinned Pieces**: Detect pieces that can't move without exposing king
//! - **Castling Rights**: Track kingside/queenside castling availability
//! - **En Passant Target**: Store valid en passant capture squares
//!
//! # Reference
//!
//! Board representation patterns from:
//! - `reference/chess_engine/src/board.rs` - Bitboard-based representation
//! - `reference/bevy-3d-chess/src/board.rs` - ECS-integrated board
//! - Chess Programming Wiki: https://www.chessprogramming.org/Board_Representation

use bevy::prelude::*;
use crate::rendering::pieces::{Piece, PieceColor};

/// Lightweight board state snapshot for move validation
///
/// Contains just enough information to validate chess moves without needing
/// access to the full ECS world. Created on-demand from ECS data, used for
/// validation, then discarded.
///
/// # Fields
///
/// - `pieces`: Vector of `(Entity, Piece, Position)` tuples representing all pieces on the board
///
/// # Representation
///
/// Positions are `(u8, u8)` tuples where:
/// - First value (rank): 0-7 (0 = rank 1, 7 = rank 8)
/// - Second value (file): 0-7 (0 = file a, 7 = file h)
///
/// Example: `(1, 4)` = e2 (starting position of white's e-pawn)
///
/// # Examples
///
/// ## Building from ECS data
///
/// ```rust,ignore
/// let mut pieces_data = Vec::new();
/// for (entity, piece, transform) in piece_query.iter() {
///     let pos = world_to_board_pos(transform.translation);
///     pieces_data.push((entity, *piece, pos));
/// }
/// let board_state = BoardState { pieces: pieces_data };
/// ```
///
/// ## Querying board state
///
/// ```rust,ignore
/// // Check if a square is empty
/// if board_state.is_empty((3, 4)) {
///     println!("e4 is empty");
/// }
///
/// // Get piece color at a position
/// match board_state.get_piece_color((1, 4)) {
///     Some(PieceColor::White) => println!("White piece on e2"),
///     Some(PieceColor::Black) => println!("Black piece on e2"),
///     None => println!("e2 is empty"),
/// }
/// ```
pub struct BoardState {
    /// All pieces currently on the board
    ///
    /// Each tuple contains:
    /// - `Entity`: Bevy entity handle (for applying moves back to ECS)
    /// - `Piece`: Piece data (type and color)
    /// - `(u8, u8)`: Board position (rank, file)
    pub pieces: Vec<(Entity, Piece, (u8, u8))>,
}

impl BoardState {
    /// Check if a given square is empty (no piece on it)
    ///
    /// This is the most frequently called method during move validation.
    /// Used to determine if a piece can move to a square and to check if
    /// paths are clear for sliding pieces (bishop, rook, queen).
    ///
    /// # Arguments
    ///
    /// * `pos` - Board position as `(rank, file)` tuple
    ///
    /// # Returns
    ///
    /// - `true` if the square is empty
    /// - `false` if there's a piece on the square
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Check if target square is empty before moving
    /// if board_state.is_empty(target_pos) {
    ///     // Square is empty, piece can move there
    /// }
    ///
    /// // Check if path is clear for sliding piece
    /// let intermediate = (3, 4);
    /// if !board_state.is_empty(intermediate) {
    ///     // Path blocked, cannot move through this square
    ///     return false;
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// O(n) where n = number of pieces on board. For better performance,
    /// use [`crate::game::resources::FastBoardState`] with O(1) bitboard lookups.
    pub fn is_empty(&self, pos: (u8, u8)) -> bool {
        !self.pieces.iter().any(|(_, _, p)| *p == pos)
    }

    /// Get the color of the piece at a given position, if any
    ///
    /// Used to determine if a square contains a friendly piece (can't capture)
    /// or an enemy piece (can capture). Essential for move validation to prevent
    /// capturing your own pieces.
    ///
    /// # Arguments
    ///
    /// * `pos` - Board position as `(rank, file)` tuple
    ///
    /// # Returns
    ///
    /// - `Some(PieceColor::White)` if a white piece is on the square
    /// - `Some(PieceColor::Black)` if a black piece is on the square
    /// - `None` if the square is empty
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Check if target square has an enemy piece (can capture)
    /// if let Some(target_color) = board_state.get_piece_color(to_pos) {
    ///     if target_color != piece_color {
    ///         // Enemy piece, capture is legal
    ///         return true;
    ///     } else {
    ///         // Friendly piece, cannot capture own pieces
    ///         return false;
    ///     }
    /// }
    /// ```
    ///
    /// # Usage in Move Validation
    ///
    /// This method is called for every potential move to enforce the rule:
    /// **"You cannot capture your own pieces"**
    pub fn get_piece_color(&self, pos: (u8, u8)) -> Option<PieceColor> {
        self.pieces
            .iter()
            .find(|(_, _, p)| *p == pos)
            .map(|(_, piece, _)| piece.color)
    }

    /// Get the piece at a specific position (type and color information)
    ///
    /// Provides full piece information rather than just color. Useful for:
    /// - Advanced move validation (castling through check)
    /// - En passant validation (checking if adjacent piece is a pawn)
    /// - Check detection (finding which piece is giving check)
    /// - Pin detection (identifying pieces that can't move)
    ///
    /// # Arguments
    ///
    /// * `pos` - Board position as `(rank, file)` tuple
    ///
    /// # Returns
    ///
    /// - `Some(&Piece)` if a piece exists at the position
    /// - `None` if the square is empty
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Check if castling path is clear (no pieces between king and rook)
    /// for square in path_squares {
    ///     if board_state.get_piece_at(square).is_some() {
    ///         // Castling blocked
    ///         return false;
    ///     }
    /// }
    ///
    /// // Validate en passant capture (adjacent piece must be pawn)
    /// if let Some(piece) = board_state.get_piece_at(adjacent_square) {
    ///     if piece.piece_type == PieceType::Pawn {
    ///         // En passant may be possible
    ///     }
    /// }
    /// ```
    ///
    /// # Future Use
    ///
    /// Currently unused but reserved for implementing:
    /// - Castling validation (checking squares king passes through aren't attacked)
    /// - En passant capture validation
    /// - Check/checkmate detection
    #[allow(dead_code)] // Reserved for future castling and en passant validation
    pub fn get_piece_at(&self, pos: (u8, u8)) -> Option<&Piece> {
        self.pieces
            .iter()
            .find(|(_, _, p)| *p == pos)
            .map(|(_, piece, _)| piece)
    }

    /// Get all pieces of a specific color
    ///
    /// Returns a vector of references to all pieces belonging to one player.
    /// Useful for:
    /// - Calculating material advantage
    /// - Detecting insufficient material (draw condition)
    /// - Generating all possible moves for a player
    /// - AI opponent move evaluation
    ///
    /// # Arguments
    ///
    /// * `color` - The color of pieces to retrieve (White or Black)
    ///
    /// # Returns
    ///
    /// Vector of references to all pieces of the specified color
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Count remaining pieces for each player
    /// let white_pieces = board_state.get_pieces_by_color(PieceColor::White);
    /// let black_pieces = board_state.get_pieces_by_color(PieceColor::Black);
    /// println!("White: {} pieces, Black: {} pieces",
    ///     white_pieces.len(), black_pieces.len());
    ///
    /// // Check for insufficient material (e.g., King vs King)
    /// if white_pieces.len() == 1 && black_pieces.len() == 1 {
    ///     // Only kings remaining, game is a draw
    ///     game_over_state = GameOverState::InsufficientMaterial;
    /// }
    ///
    /// // Generate all possible moves for white
    /// for piece in board_state.get_pieces_by_color(PieceColor::White) {
    ///     let moves = get_possible_moves(piece.piece_type, piece.color, ...);
    /// }
    /// ```
    ///
    /// # Performance Note
    ///
    /// This iterates through all pieces and filters by color, so it's O(n).
    /// If called frequently (e.g., every frame for AI), consider caching the result.
    pub fn get_pieces_by_color(&self, color: PieceColor) -> Vec<&Piece> {
        self.pieces
            .iter()
            .filter(|(_, piece, _)| piece.color == color)
            .map(|(_, piece, _)| piece)
            .collect()
    }

    /// Get the total number of pieces on the board
    ///
    /// Simple utility to check how many pieces remain in play.
    ///
    /// # Returns
    ///
    /// Total count of all pieces (both colors)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// println!("Pieces remaining: {}", board_state.piece_count());
    /// // Starting game: 32 pieces
    /// // Late endgame: might be 4-6 pieces
    /// ```
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    /// Get all positions occupied by pieces
    ///
    /// Returns a vector of all board positions that have pieces on them.
    /// Useful for visualization and debugging.
    ///
    /// # Returns
    ///
    /// Vector of positions `(rank, file)` where pieces are located
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// for pos in board_state.occupied_squares() {
    ///     println!("Piece at ({}, {})", pos.0, pos.1);
    /// }
    /// ```
    pub fn occupied_squares(&self) -> Vec<(u8, u8)> {
        self.pieces.iter().map(|(_, _, pos)| *pos).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::pieces::PieceType;
    use bevy::ecs::entity::EntityRow;

    // Helper to create test entities
    fn create_test_entity(index: u32) -> Entity {
        Entity::from_row(EntityRow::from_raw_u32(index).unwrap())
    }

    #[test]
    fn test_is_empty_on_empty_square() {
        //! Verifies is_empty returns true for squares with no pieces
        let board = BoardState {
            pieces: vec![(
                create_test_entity(0),
                Piece {
                    color: PieceColor::White,
                    piece_type: PieceType::Pawn,
                    x: 1,
                    y: 1,
                },
                (1, 1),
            )],
        };

        assert!(board.is_empty((0, 0)));
        assert!(board.is_empty((7, 7)));
        assert!(!board.is_empty((1, 1)));
    }

    #[test]
    fn test_get_piece_color_returns_correct_color() {
        //! Tests retrieving piece color from occupied squares
        let board = BoardState {
            pieces: vec![
                (
                    create_test_entity(0),
                    Piece {
                        color: PieceColor::White,
                        piece_type: PieceType::Pawn,
                        x: 1,
                        y: 0,
                    },
                    (1, 0),
                ),
                (
                    create_test_entity(1),
                    Piece {
                        color: PieceColor::Black,
                        piece_type: PieceType::Pawn,
                        x: 6,
                        y: 0,
                    },
                    (6, 0),
                ),
            ],
        };

        assert_eq!(board.get_piece_color((1, 0)), Some(PieceColor::White));
        assert_eq!(board.get_piece_color((6, 0)), Some(PieceColor::Black));
        assert_eq!(board.get_piece_color((3, 3)), None);
    }

    #[test]
    fn test_get_piece_at_returns_piece() {
        //! Verifies get_piece_at returns full piece information
        let piece = Piece {
            color: PieceColor::White,
            piece_type: PieceType::Queen,
            x: 3,
            y: 4,
        };

        let board = BoardState {
            pieces: vec![(create_test_entity(0), piece, (3, 4))],
        };

        let retrieved = board.get_piece_at((3, 4)).unwrap();
        assert_eq!(retrieved.piece_type, PieceType::Queen);
        assert_eq!(retrieved.color, PieceColor::White);
    }

    #[test]
    fn test_get_pieces_by_color_filters_correctly() {
        //! Tests filtering pieces by color
        let board = BoardState {
            pieces: vec![
                (
                    create_test_entity(0),
                    Piece {
                        color: PieceColor::White,
                        piece_type: PieceType::Pawn,
                        x: 1,
                        y: 0,
                    },
                    (1, 0),
                ),
                (
                    create_test_entity(1),
                    Piece {
                        color: PieceColor::White,
                        piece_type: PieceType::Rook,
                        x: 0,
                        y: 0,
                    },
                    (0, 0),
                ),
                (
                    create_test_entity(2),
                    Piece {
                        color: PieceColor::Black,
                        piece_type: PieceType::Pawn,
                        x: 6,
                        y: 0,
                    },
                    (6, 0),
                ),
            ],
        };

        let white_pieces = board.get_pieces_by_color(PieceColor::White);
        let black_pieces = board.get_pieces_by_color(PieceColor::Black);

        assert_eq!(white_pieces.len(), 2);
        assert_eq!(black_pieces.len(), 1);
    }

    #[test]
    fn test_piece_count() {
        //! Verifies piece_count returns correct total
        let board = BoardState {
            pieces: vec![
                (
                    create_test_entity(0),
                    Piece {
                        color: PieceColor::White,
                        piece_type: PieceType::Pawn,
                        x: 1,
                        y: 1,
                    },
                    (1, 1),
                ),
                (
                    create_test_entity(1),
                    Piece {
                        color: PieceColor::Black,
                        piece_type: PieceType::Pawn,
                        x: 6,
                        y: 6,
                    },
                    (6, 6),
                ),
            ],
        };

        assert_eq!(board.piece_count(), 2);
    }

    #[test]
    fn test_occupied_squares() {
        //! Tests retrieving all occupied positions
        let board = BoardState {
            pieces: vec![
                (
                    create_test_entity(0),
                    Piece {
                        color: PieceColor::White,
                        piece_type: PieceType::Pawn,
                        x: 1,
                        y: 4,
                    },
                    (1, 4),
                ),
                (
                    create_test_entity(1),
                    Piece {
                        color: PieceColor::Black,
                        piece_type: PieceType::Knight,
                        x: 7,
                        y: 7,
                    },
                    (7, 7),
                ),
            ],
        };

        let occupied = board.occupied_squares();
        assert_eq!(occupied.len(), 2);
        assert!(occupied.contains(&(1, 4)));
        assert!(occupied.contains(&(7, 7)));
    }
}
