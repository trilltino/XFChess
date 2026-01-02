//! Move history tracking resource
//!
//! Maintains a complete chronological record of all moves made during the game.
//! This enables critical features like:
//!
//! - **Undo/Redo**: Restore previous game states
//! - **PGN Export**: Save games in Portable Game Notation format
//! - **Move Review**: Let players analyze their game afterward
//! - **Three-fold Repetition**: Detect draw conditions automatically
//!
//! # Architecture
//!
//! MoveHistory stores a `Vec<MoveRecord>` where each record contains:
//! - Piece moved (type and color)
//! - From/to positions
//! - Special move flags (castling, en passant, check, checkmate)
//! - Captured piece (if any)
//!
//! # Integration
//!
//! Updated by [`crate::game::systems::game_logic`] after each move validation.
//! Read by UI systems to display move notation and game review features.
//!
//! # Reference
//!
//! - `reference/chess_engine/src/types.rs` - Move representation patterns
//! - PGN specification: https://en.wikipedia.org/wiki/Portable_Game_Notation

use crate::game::components::MoveRecord;
use bevy::prelude::*;

/// Resource storing the complete move history for the current game
///
/// # Fields
///
/// - `moves`: Ordered vector of all moves made since game start
///
/// # Examples
///
/// ## Recording moves
///
/// ```rust,ignore
/// fn execute_move_system(mut history: ResMut<MoveHistory>) {
///     let move_record = MoveRecord {
///         piece_type: PieceType::Pawn,
///         piece_color: PieceColor::White,
///         from: (4, 1),
///         to: (4, 3),
///         captured: None,
///         is_castling: false,
///         is_en_passant: false,
///         is_check: false,
///         is_checkmate: false,
///     };
///
///     history.add_move(move_record);
/// }
/// ```
///
/// ## Reviewing history
///
/// ```rust,ignore
/// fn display_last_move(history: Res<MoveHistory>) {
///     if let Some(last) = history.last_move() {
///         println!("Last move: {:?} to {:?}", last.from, last.to);
///     }
/// }
/// ```
#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct MoveHistory {
    /// Chronological list of all moves made in the game
    ///
    /// Index 0 = Move 1 (White's first move)
    /// Index 1 = Move 1 (Black's response)
    /// Index 2 = Move 2 (White's move)
    /// etc.
    pub moves: Vec<MoveRecord>,
}

impl MoveHistory {
    /// Add a new move to the history
    ///
    /// Appends the move record to the end of the move list. Should be called
    /// after move validation succeeds but before switching turns.
    ///
    /// # Arguments
    ///
    /// * `record` - The move record to add
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// history.add_move(MoveRecord {
    ///     piece_type: PieceType::Knight,
    ///     piece_color: PieceColor::Black,
    ///     from: (1, 7),
    ///     to: (2, 5),
    ///     captured: None,
    ///     is_castling: false,
    ///     is_en_passant: false,
    ///     is_check: true,  // Knight gives check
    ///     is_checkmate: false,
    /// });
    /// ```
    pub fn add_move(&mut self, record: MoveRecord) {
        self.moves.push(record);
    }

    /// Get the most recent move, if any
    ///
    /// Returns `None` if the game just started and no moves have been made yet.
    /// Useful for detecting en passant opportunities and displaying last move UI.
    ///
    /// # Returns
    ///
    /// - `Some(&MoveRecord)` - Reference to the last move made
    /// - `None` - Game just started, no moves yet
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Check if last move was a pawn double-push (for en passant)
    /// if let Some(last) = history.last_move() {
    ///     if last.piece_type == PieceType::Pawn {
    ///         let distance = (last.to.1 as i8 - last.from.1 as i8).abs();
    ///         if distance == 2 {
    ///             println!("En passant may be available!");
    ///         }
    ///     }
    /// }
    /// ```
    pub fn last_move(&self) -> Option<&MoveRecord> {
        self.moves.last()
    }

    /// Get the total number of half-moves (ply) made
    ///
    /// In chess, a "ply" or "half-move" is one player's move. Two ply = one full move.
    /// This is useful for:
    /// - Fifty-move rule (draw after 50 moves with no captures or pawn moves)
    /// - Calculating game progress
    ///
    /// # Returns
    ///
    /// Number of half-moves (0 at game start)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let ply_count = history.len();
    /// let full_moves = (ply_count / 2) + 1;
    /// println!("Move {} (ply {})", full_moves, ply_count);
    /// ```
    pub fn len(&self) -> usize {
        self.moves.len()
    }

    /// Check if move history is empty (no moves made yet)
    ///
    /// # Returns
    ///
    /// `true` if game just started, `false` if at least one move has been made
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if history.is_empty() {
    ///     println!("Game just started!");
    /// }
    /// ```
    #[allow(dead_code)] // Public API - useful for UI and game logic
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// Clear all move history (for starting a new game)
    ///
    /// Removes all moves from the history. Should be called when starting
    /// a new game or loading a saved game.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn start_new_game(mut history: ResMut<MoveHistory>) {
    ///     history.clear();
    /// }
    /// ```
    pub fn clear(&mut self) {
        self.moves.clear();
    }

    /// Get a specific move by index (ply number)
    ///
    /// # Arguments
    ///
    /// * `index` - Zero-based ply index (0 = first move)
    ///
    /// # Returns
    ///
    /// - `Some(&MoveRecord)` - Move at that index
    /// - `None` - Index out of bounds
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Get the opening move (1. e4 or similar)
    /// if let Some(first_move) = history.get_move(0) {
    ///     println!("White opened with: {:?}", first_move);
    /// }
    /// ```
    #[allow(dead_code)] // Public API - useful for UI and game analysis
    pub fn get_move(&self, index: usize) -> Option<&MoveRecord> {
        self.moves.get(index)
    }

    /// Iterate over all moves in chronological order
    ///
    /// Useful for generating PGN notation or analyzing the full game.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// for (i, move_record) in history.iter().enumerate() {
    ///     println!("Move {}: {:?}", i + 1, move_record);
    /// }
    /// ```
    #[allow(dead_code)] // Public API - useful for UI and game analysis
    pub fn iter(&self) -> std::slice::Iter<'_, MoveRecord> {
        self.moves.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::pieces::{PieceColor, PieceType};

    #[test]
    fn test_move_history_default() {
        //! Verifies MoveHistory starts empty
        let history = MoveHistory::default();

        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.last_move().is_none());
    }

    #[test]
    fn test_add_move() {
        //! Tests adding a single move to history
        let mut history = MoveHistory::default();

        let move_record = MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::White,
            from: (4, 1),
            to: (4, 3),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        history.add_move(move_record);

        assert_eq!(history.len(), 1);
        assert!(!history.is_empty());
        assert!(history.last_move().is_some());
    }

    #[test]
    fn test_last_move_returns_correct_move() {
        //! Tests that last_move returns the most recent move
        let mut history = MoveHistory::default();

        let first_move = MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::White,
            from: (4, 1),
            to: (4, 3),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        let second_move = MoveRecord {
            piece_type: PieceType::Knight,
            piece_color: PieceColor::Black,
            from: (1, 7),
            to: (2, 5),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        history.add_move(first_move);
        history.add_move(second_move);

        let last = history.last_move().unwrap();
        assert_eq!(last.piece_type, PieceType::Knight);
        assert_eq!(last.piece_color, PieceColor::Black);
    }

    #[test]
    fn test_len_increments_correctly() {
        //! Tests that length increases with each move
        let mut history = MoveHistory::default();

        assert_eq!(history.len(), 0);

        for i in 1..=10 {
            history.add_move(MoveRecord {
                piece_type: PieceType::Pawn,
                piece_color: if i % 2 == 1 {
                    PieceColor::White
                } else {
                    PieceColor::Black
                },
                from: (i as u8 % 8, 1),
                to: (i as u8 % 8, 3),
                captured: None,
                is_castling: false,
                is_en_passant: false,
                is_check: false,
                is_checkmate: false,
            });

            assert_eq!(history.len(), i);
        }
    }

    #[test]
    fn test_clear_removes_all_moves() {
        //! Tests clearing move history
        let mut history = MoveHistory::default();

        // Add several moves
        for _ in 0..5 {
            history.add_move(MoveRecord {
                piece_type: PieceType::Pawn,
                piece_color: PieceColor::White,
                from: (0, 1),
                to: (0, 3),
                captured: None,
                is_castling: false,
                is_en_passant: false,
                is_check: false,
                is_checkmate: false,
            });
        }

        assert_eq!(history.len(), 5);

        history.clear();

        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.last_move().is_none());
    }

    #[test]
    fn test_get_move_by_index() {
        //! Tests retrieving specific moves by index
        let mut history = MoveHistory::default();

        let move1 = MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::White,
            from: (4, 1),
            to: (4, 3),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        let move2 = MoveRecord {
            piece_type: PieceType::Knight,
            piece_color: PieceColor::Black,
            from: (1, 7),
            to: (2, 5),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        history.add_move(move1);
        history.add_move(move2);

        // Check first move
        let retrieved_move1 = history.get_move(0).unwrap();
        assert_eq!(retrieved_move1.piece_type, PieceType::Pawn);
        assert_eq!(retrieved_move1.from, (4, 1));

        // Check second move
        let retrieved_move2 = history.get_move(1).unwrap();
        assert_eq!(retrieved_move2.piece_type, PieceType::Knight);
        assert_eq!(retrieved_move2.from, (1, 7));

        // Check out of bounds
        assert!(history.get_move(2).is_none());
    }

    #[test]
    fn test_iter_returns_all_moves() {
        //! Tests iterating over move history
        let mut history = MoveHistory::default();

        // Add 3 moves
        for i in 0..3 {
            history.add_move(MoveRecord {
                piece_type: PieceType::Pawn,
                piece_color: if i % 2 == 0 {
                    PieceColor::White
                } else {
                    PieceColor::Black
                },
                from: (i, 1),
                to: (i, 3),
                captured: None,
                is_castling: false,
                is_en_passant: false,
                is_check: false,
                is_checkmate: false,
            });
        }

        let mut count = 0;
        for (i, move_record) in history.iter().enumerate() {
            assert_eq!(move_record.from.0, i as u8);
            count += 1;
        }

        assert_eq!(count, 3);
    }

    #[test]
    fn test_move_history_with_captures() {
        //! Tests history tracking moves with captures
        let mut history = MoveHistory::default();

        let capture_move = MoveRecord {
            piece_type: PieceType::Queen,
            piece_color: PieceColor::White,
            from: (3, 4),
            to: (7, 4),
            captured: Some(PieceType::Rook),
            is_castling: false,
            is_en_passant: false,
            is_check: true,
            is_checkmate: false,
        };

        history.add_move(capture_move);

        let last = history.last_move().unwrap();
        assert_eq!(last.captured, Some(PieceType::Rook));
        assert!(last.is_check);
    }

    #[test]
    fn test_move_history_with_special_moves() {
        //! Tests history tracking special moves (castling, en passant, checkmate)
        let mut history = MoveHistory::default();

        // Castling
        history.add_move(MoveRecord {
            piece_type: PieceType::King,
            piece_color: PieceColor::White,
            from: (0, 4),
            to: (0, 6),
            captured: None,
            is_castling: true,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        });

        // En passant
        history.add_move(MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::Black,
            from: (4, 3),
            to: (3, 2),
            captured: Some(PieceType::Pawn),
            is_castling: false,
            is_en_passant: true,
            is_check: false,
            is_checkmate: false,
        });

        // Checkmate
        history.add_move(MoveRecord {
            piece_type: PieceType::Queen,
            piece_color: PieceColor::White,
            from: (3, 4),
            to: (5, 6),
            captured: Some(PieceType::Pawn),
            is_castling: false,
            is_en_passant: false,
            is_check: true,
            is_checkmate: true,
        });

        assert_eq!(history.len(), 3);
        assert!(history.get_move(0).unwrap().is_castling);
        assert!(history.get_move(1).unwrap().is_en_passant);
        assert!(history.get_move(2).unwrap().is_checkmate);
    }

    #[test]
    fn test_realistic_game_opening() {
        //! Tests recording a realistic game opening (1. e4 e5 2. Nf3)
        let mut history = MoveHistory::default();

        // 1. e4
        history.add_move(MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::White,
            from: (1, 4),
            to: (3, 4),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        });

        // 1... e5
        history.add_move(MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::Black,
            from: (6, 4),
            to: (4, 4),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        });

        // 2. Nf3
        history.add_move(MoveRecord {
            piece_type: PieceType::Knight,
            piece_color: PieceColor::White,
            from: (0, 6),
            to: (2, 5),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        });

        assert_eq!(history.len(), 3, "Should have recorded 3 half-moves");

        let first_move = history.get_move(0).unwrap();
        assert_eq!(first_move.piece_type, PieceType::Pawn);
        assert_eq!(first_move.piece_color, PieceColor::White);
        assert_eq!(first_move.from, (1, 4));
        assert_eq!(first_move.to, (3, 4));
    }
}
