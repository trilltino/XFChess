//! Chess engine resource - Unified engine state management
//!
//! This module provides the `ChessEngine` resource that wraps the chess_engine crate's
//! `Game` struct, making it the single source of truth for all chess logic.
//!
//! # Architecture
//!
//! The chess engine is authoritative for:
//! - Move validation
//! - Legal move generation
//! - Check/checkmate detection
//! - Position evaluation
//!
//! ECS is used for:
//! - Rendering (piece positions, visual feedback)
//! - User interaction (click handling, selection)
//! - UI state (timers, move history display)
//!
//! # Synchronization
//!
//! Bidirectional sync ensures consistency:
//! - **ECS → Engine**: Before validation/computation (sync current board state)
//! - **Engine → ECS**: After moves (update piece positions for rendering)
//!
//! # Coordinate System
//!
//! - **ECS coordinates**: `(x, y)` where x=rank (0-7), y=file (0-7)
//! - **Engine coordinates**: Linear index 0-63 where index = y * 8 + x
//! - Conversion helpers: `square_to_index()` and `index_to_square()`

use crate::game::components::HasMoved;
use crate::game::resources::turn::CurrentTurn;
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::prelude::*;
use chess_engine::Color;
use chess_engine::{generate_pseudo_legal_moves, is_legal_move, new_game, reset_game, Game};

/// Chess engine resource - Single source of truth for chess game state
///
/// Wraps the chess_engine crate's `Game` struct, providing unified access
/// to move validation, legal move generation, and game state queries.
///
/// # Usage
///
/// ```rust,ignore
/// fn validate_move(
///     engine: Res<ChessEngine>,
///     from: (u8, u8),
///     to: (u8, u8),
///     color: PieceColor,
/// ) -> bool {
///     let src = ChessEngine::square_to_index(from.0, from.1);
///     let dst = ChessEngine::square_to_index(to.0, to.1);
///     let engine_color = ChessEngine::piece_color_to_engine(color);
///     is_legal_move(&engine.game, src, dst, engine_color)
/// }
/// ```
#[derive(Resource)]
pub struct ChessEngine {
    /// The underlying chess engine game state
    ///
    /// This is the authoritative source for all chess logic.
    /// ECS components are synchronized with this state.
    pub game: Game,
}

impl Default for ChessEngine {
    fn default() -> Self {
        Self { game: new_game() }
    }
}

impl ChessEngine {
    /// Convert board coordinates to engine square index
    ///
    /// # Arguments
    ///
    /// * `x` - Rank (0-7, where 0 is rank 1, 7 is rank 8)
    /// * `y` - File (0-7, where 0 is file a, 7 is file h)
    ///
    /// # Returns
    ///
    /// Engine square index (0-63) where:
    /// - 0 = a1 (x=0, y=0)
    /// - 7 = h1 (x=0, y=7)
    /// - 56 = a8 (x=7, y=0)
    /// - 63 = h8 (x=7, y=7)
    ///
    /// # Formula
    ///
    /// `index = x * 8 + y` (x=rank, y=file)
    #[inline]
    pub fn square_to_index(x: u8, y: u8) -> i8 {
        (x * 8 + y) as i8
    }

    /// Convert engine square index to board coordinates
    ///
    /// # Arguments
    ///
    /// * `index` - Engine square index (0-63)
    ///
    /// # Returns
    ///
    /// Board coordinates `(x, y)` where:
    /// - x = rank (0-7, where 0=rank 1, 7=rank 8)
    /// - y = file (0-7, where 0=file a, 7=file h)
    #[inline]
    pub fn index_to_square(index: i8) -> (u8, u8) {
        let idx = index as u8;
        (idx / 8, idx % 8)
    }

    /// Convert PieceColor to engine Color
    ///
    /// # Arguments
    ///
    /// * `color` - ECS PieceColor enum
    ///
    /// # Returns
    ///
    /// Engine Color (1 for White, -1 for Black)
    #[inline]
    pub fn piece_color_to_engine(color: PieceColor) -> Color {
        match color {
            PieceColor::White => 1,
            PieceColor::Black => -1,
        }
    }

    /// Convert PieceType to engine piece ID
    ///
    /// # Arguments
    ///
    /// * `piece_type` - ECS PieceType enum
    ///
    /// # Returns
    ///
    /// Engine piece ID (1-6):
    /// - 1 = Pawn
    /// - 2 = Knight
    /// - 3 = Bishop
    /// - 4 = Rook
    /// - 5 = Queen
    /// - 6 = King
    #[inline]
    pub fn piece_type_to_id(piece_type: PieceType) -> i8 {
        match piece_type {
            PieceType::Pawn => 1,
            PieceType::Knight => 2,
            PieceType::Bishop => 3,
            PieceType::Rook => 4,
            PieceType::Queen => 5,
            PieceType::King => 6,
        }
    }

    /// Synchronize ECS board state to engine
    ///
    /// Copies all piece positions, castling rights, and move counter from ECS
    /// to the engine's internal board representation.
    ///
    /// # Arguments
    ///
    /// * `pieces_query` - Query for all pieces with HasMoved component (may include Transform)
    /// * `current_turn` - Current turn resource for move counter
    ///
    /// # Side Effects
    ///
    /// Modifies `self.game.board` and castling flags to match ECS state.
    pub fn sync_ecs_to_engine(
        &mut self,
        pieces_query: &Query<(Entity, &Piece, &HasMoved)>,
        current_turn: &CurrentTurn,
    ) {
        self.sync_ecs_to_engine_impl(pieces_query.iter().map(|(e, p, h)| (e, p, h)), current_turn);
    }

    /// Synchronize ECS board state to engine (overload for queries with Transform)
    ///
    /// This version handles queries that include Transform component, which can happen
    /// in observer functions. It extracts just the needed data and delegates to the
    /// internal implementation.
    pub fn sync_ecs_to_engine_with_transform(
        &mut self,
        pieces_query: &Query<(Entity, &Piece, &HasMoved, &Transform)>,
        current_turn: &CurrentTurn,
    ) {
        self.sync_ecs_to_engine_impl(
            pieces_query.iter().map(|(e, p, h, _)| (e, p, h)),
            current_turn,
        );
    }

    /// Internal implementation that works with any iterator over piece data
    pub fn sync_ecs_to_engine_impl<'a>(
        &mut self,
        pieces: impl Iterator<Item = (Entity, &'a Piece, &'a HasMoved)>,
        current_turn: &CurrentTurn,
    ) {
        // Clear the board
        self.game.board = [0; 64];

        // Copy all pieces to engine board AND synchronize castling rights in one pass
        for (_, piece, has_moved) in pieces {
            let square = Self::square_to_index(piece.x, piece.y) as usize;

            // Copy piece to board
            let piece_id = Self::piece_type_to_id(piece.piece_type);

            self.game.board[square] = if piece.color == PieceColor::White {
                piece_id
            } else {
                -piece_id
            };

            // Synchronize castling rights
            if piece.piece_type == PieceType::King {
                if piece.color == PieceColor::White {
                    self.game.white_king_has_moved = has_moved.moved;
                } else {
                    self.game.black_king_has_moved = has_moved.moved;
                }
            } else if piece.piece_type == PieceType::Rook {
                // Check starting positions for rooks
                match (piece.color, square) {
                    (PieceColor::White, 0) => self.game.white_rook_0_has_moved = has_moved.moved,
                    (PieceColor::White, 7) => self.game.white_rook_7_has_moved = has_moved.moved,
                    (PieceColor::Black, 56) => self.game.black_rook_56_has_moved = has_moved.moved,
                    (PieceColor::Black, 63) => self.game.black_rook_63_has_moved = has_moved.moved,
                    _ => {} // Rook not in starting position, doesn't affect castling
                }
            }
        }

        // Synchronize move counter
        // Chess engines use 0-indexed move counter, so subtract 1
        self.game.move_counter = (current_turn.move_number - 1) as i32;
    }

    /// Synchronize engine board state to ECS
    ///
    /// Updates piece positions in ECS to match the engine's board state.
    /// Handles captures (despawns pieces not in engine) and position updates.
    ///
    /// # Arguments
    ///
    /// * `commands` - Bevy Commands for entity manipulation
    /// * `pieces_query` - Query for all pieces with mutable access
    ///
    /// # Side Effects
    ///
    /// - Updates Piece component positions
    /// - Updates HasMoved component based on castling flags
    /// - Despawns captured pieces
    ///
    /// # Note
    ///
    /// This function does NOT spawn new pieces. Pieces should already exist in ECS.
    /// If the engine has pieces that don't exist in ECS, they will be ignored.
    pub fn sync_engine_to_ecs(
        &self,
        commands: &mut Commands,
        pieces_query: &mut Query<(Entity, &mut Piece, &mut HasMoved)>,
    ) {
        // Build a map of engine board positions
        let mut engine_positions: Vec<(u8, u8, PieceColor, PieceType)> = Vec::new();

        for (index, &piece_id) in self.game.board.iter().enumerate() {
            if piece_id != 0 {
                let (x, y) = Self::index_to_square(index as i8);
                let color = if piece_id > 0 {
                    PieceColor::White
                } else {
                    PieceColor::Black
                };
                let piece_type = match piece_id.abs() {
                    1 => PieceType::Pawn,
                    2 => PieceType::Knight,
                    3 => PieceType::Bishop,
                    4 => PieceType::Rook,
                    5 => PieceType::Queen,
                    6 => PieceType::King,
                    _ => continue, // Invalid piece ID
                };
                engine_positions.push((x, y, color, piece_type));
            }
        }

        // Update ECS pieces to match engine positions
        let mut pieces_to_despawn = Vec::new();

        for (entity, mut piece, mut has_moved) in pieces_query.iter_mut() {
            // Find matching piece in engine
            let engine_piece = engine_positions.iter().find(|(x, y, c, t)| {
                *x == piece.x && *y == piece.y && *c == piece.color && *t == piece.piece_type
            });

            if let Some((new_x, new_y, _, _)) = engine_piece {
                // Piece still exists, update position if changed
                if piece.x != *new_x || piece.y != *new_y {
                    piece.x = *new_x;
                    piece.y = *new_y;
                }

                // Update castling rights from engine
                if piece.piece_type == PieceType::King {
                    let king_moved = match piece.color {
                        PieceColor::White => self.game.white_king_has_moved,
                        PieceColor::Black => self.game.black_king_has_moved,
                    };
                    has_moved.moved = king_moved;
                } else if piece.piece_type == PieceType::Rook {
                    let rook_moved = match (
                        piece.color,
                        Self::square_to_index(piece.x, piece.y) as usize,
                    ) {
                        (PieceColor::White, 0) => self.game.white_rook_0_has_moved,
                        (PieceColor::White, 7) => self.game.white_rook_7_has_moved,
                        (PieceColor::Black, 56) => self.game.black_rook_56_has_moved,
                        (PieceColor::Black, 63) => self.game.black_rook_63_has_moved,
                        _ => has_moved.moved, // Keep current state for rooks not in starting positions
                    };
                    has_moved.moved = rook_moved;
                }
            } else {
                // Piece not found in engine board - it was captured
                pieces_to_despawn.push(entity);
            }
        }

        // Despawn captured pieces
        for entity in pieces_to_despawn {
            commands.entity(entity).despawn();
        }
    }

    /// Reset the engine to starting position
    ///
    /// Calls the engine's reset_game function to restore initial board state.
    pub fn reset(&mut self) {
        reset_game(&mut self.game);
    }

    /// Get all legal moves for a piece at a specific square
    ///
    /// Generates all pseudo-legal moves for the given color, filters by source square,
    /// and validates each move to ensure it doesn't leave the king in check.
    ///
    /// # Arguments
    ///
    /// * `square` - Board coordinates `(x, y)` where the piece is located
    /// * `color` - The color of the piece (White or Black)
    ///
    /// # Returns
    ///
    /// Vector of legal destination squares `(x, y)` that the piece can move to.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let legal_moves = engine.get_legal_moves_for_square((1, 4), PieceColor::White);
    /// // Returns: [(2, 4), (3, 4)] for a white pawn on e2
    /// ```
    pub fn get_legal_moves_for_square(
        &mut self,
        square: (u8, u8),
        color: PieceColor,
    ) -> Vec<(u8, u8)> {
        let src_index = Self::square_to_index(square.0, square.1);
        let engine_color = Self::piece_color_to_engine(color);

        // Generate all pseudo-legal moves for this color
        let pseudo_legal_moves = generate_pseudo_legal_moves(&self.game, engine_color);

        // Filter moves that start from this square and validate each one
        let mut legal_moves = Vec::new();
        for mv in pseudo_legal_moves {
            if mv.src == src_index {
                // Validate that this move doesn't leave king in check
                if is_legal_move(&mut self.game, mv.src, mv.dst, engine_color) {
                    let (dst_x, dst_y) = Self::index_to_square(mv.dst);
                    legal_moves.push((dst_x, dst_y));
                }
            }
        }

        legal_moves
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_square_to_index() {
        // x=rank, y=file, formula: x * 8 + y
        assert_eq!(ChessEngine::square_to_index(0, 0), 0); // a1: rank=0, file=0
        assert_eq!(ChessEngine::square_to_index(0, 7), 7); // h1: rank=0, file=7
        assert_eq!(ChessEngine::square_to_index(7, 0), 56); // a8: rank=7, file=0
        assert_eq!(ChessEngine::square_to_index(7, 7), 63); // h8: rank=7, file=7
        assert_eq!(ChessEngine::square_to_index(3, 4), 28); // e4: rank=3, file=4
    }

    #[test]
    fn test_index_to_square() {
        // Returns (rank, file)
        assert_eq!(ChessEngine::index_to_square(0), (0, 0)); // a1
        assert_eq!(ChessEngine::index_to_square(7), (0, 7)); // h1
        assert_eq!(ChessEngine::index_to_square(56), (7, 0)); // a8
        assert_eq!(ChessEngine::index_to_square(63), (7, 7)); // h8
        assert_eq!(ChessEngine::index_to_square(28), (3, 4)); // e4
    }

    #[test]
    fn test_coordinate_roundtrip() {
        // Test that square_to_index and index_to_square are inverses
        for x in 0..8 {
            for y in 0..8 {
                let index = ChessEngine::square_to_index(x, y);
                let (x2, y2) = ChessEngine::index_to_square(index);
                assert_eq!((x, y), (x2, y2), "Roundtrip failed for ({}, {})", x, y);
            }
        }
    }

    #[test]
    fn test_piece_color_to_engine() {
        assert_eq!(ChessEngine::piece_color_to_engine(PieceColor::White), 1);
        assert_eq!(ChessEngine::piece_color_to_engine(PieceColor::Black), -1);
    }

    #[test]
    fn test_piece_type_to_id() {
        assert_eq!(ChessEngine::piece_type_to_id(PieceType::Pawn), 1);
        assert_eq!(ChessEngine::piece_type_to_id(PieceType::Knight), 2);
        assert_eq!(ChessEngine::piece_type_to_id(PieceType::Bishop), 3);
        assert_eq!(ChessEngine::piece_type_to_id(PieceType::Rook), 4);
        assert_eq!(ChessEngine::piece_type_to_id(PieceType::Queen), 5);
        assert_eq!(ChessEngine::piece_type_to_id(PieceType::King), 6);
    }

    #[test]
    fn test_chess_engine_default() {
        let engine = ChessEngine::default();
        // Engine should be initialized with starting position
        // Check that board has pieces (not all zeros)
        let has_pieces = engine.game.board.iter().any(|&p| p != 0);
        assert!(has_pieces, "Default engine should have pieces on board");
    }
}
