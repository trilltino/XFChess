//! Game phase tracking and move recording components
//!
//! This module defines components that track the overall game state and individual move history.
//! These are distinct from [`crate::core::GameState`] (LaunchMenu vs Multiplayer) - this tracks
//! chess-specific phases within active gameplay.
//!
//! # Components
//!
//! - **[`GamePhase`]**: Current phase of the chess game (Setup/Playing/Check/Checkmate/Stalemate)
//! - **[`MoveRecord`]**: Complete record of a single chess move with all metadata
//!
//! # Architecture Notes
//!
//! [`GamePhase`] is a component rather than a resource to allow:
//! - Multiple simultaneous games (future multiplayer/spectator modes)
//! - Easy state serialization per-game
//! - Clean ECS queries like `Query<&GamePhase, With<ActiveGame>>`
//!
//! # Integration
//!
//! Used by:
//! - [`crate::game::systems::game_logic`] - Detects check/checkmate/stalemate
//! - [`crate::game::resources::MoveHistory`] - Stores MoveRecord sequences
//! - [`crate::ui::game_ui`] - Displays current phase to player
//!
//! # Reference
//!
//! Game phase patterns inspired by:
//! - `reference/chess_engine/src/types.rs` - Engine game state representation
//! - `reference/bevy-3d-chess/src/board.rs` - ECS game state tracking
//!
//! # Examples
//!
//! ## Checking game phase
//!
//! ```rust
//! use bevy::prelude::*;
//! use xfchess::game::components::GamePhase;
//!
//! fn display_game_status(phase: Res<GamePhase>) {
//!     match *phase {
//!         GamePhase::Check => println!("Check!"),
//!         GamePhase::Checkmate => println!("Checkmate!"),
//!         GamePhase::Stalemate => println!("Draw by stalemate"),
//!         _ => {}
//!     }
//! }
//! ```
//!
//! ## Recording a move
//!
//! ```rust
//! use xfchess::game::components::MoveRecord;
//! use xfchess::rendering::pieces::{PieceType, PieceColor};
//!
//! let move_record = MoveRecord {
//!     piece_type: PieceType::Pawn,
//!     piece_color: PieceColor::White,
//!     from: (4, 1), // e2
//!     to: (4, 3),   // e4
//!     captured: None,
//!     is_castling: false,
//!     is_en_passant: false,
//!     is_check: false,
//!     is_checkmate: false,
//! };
//! ```

use bevy::prelude::*;
use crate::rendering::pieces::{PieceColor, PieceType};

/// Current phase of an active chess game
///
/// Tracks the progression of a chess game from initial setup through to conclusion.
/// This is separate from [`crate::core::GameState`] which controls app-level state
/// (menu vs gameplay).
///
/// # Phase Flow
///
/// ```text
/// Setup → Playing ⇄ Check → Checkmate/Stalemate
/// ```
///
/// # Phase Descriptions
///
/// - **Setup**: Pieces are being placed on the board (initial load)
/// - **Playing**: Normal play, neither player in check
/// - **Check**: Current player's king is under attack (must respond)
/// - **Checkmate**: Current player's king is in check with no legal moves (game over)
/// - **Stalemate**: Current player has no legal moves but isn't in check (draw)
///
/// # Usage
///
/// Systems can query the phase to enable/disable functionality:
///
/// ```rust
/// use bevy::prelude::*;
/// use xfchess::game::components::GamePhase;
///
/// fn allow_moves_system(phase: Res<GamePhase>) -> bool {
///     matches!(*phase, GamePhase::Playing | GamePhase::Check)
/// }
/// ```
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub enum GamePhase {
    /// Initial board setup phase
    ///
    /// Pieces are being spawned and positioned. Player input disabled until
    /// transition to Playing.
    #[default]
    Setup,

    /// Active gameplay with no check condition
    ///
    /// Both players can make any legal move. Most common state during a game.
    Playing,

    /// Current player's king is under attack
    ///
    /// Player must make a move that:
    /// - Moves the king out of check
    /// - Blocks the attacking piece
    /// - Captures the attacking piece
    Check,

    /// Game over: Current player is in check with no legal moves
    ///
    /// Attacking player wins. Game cannot continue.
    Checkmate,

    /// Game over: Current player has no legal moves but is not in check
    ///
    /// Result is a draw. Common in endgames with insufficient material.
    Stalemate,
}

/// Complete record of a single chess move with all metadata
///
/// Stores everything needed to:
/// - Display the move in algebraic notation (e4, Nf3, O-O, etc.)
/// - Animate the move visually
/// - Implement undo/redo functionality
/// - Analyze the game (detect threats, evaluate positions)
/// - Export to PGN (Portable Game Notation) format
///
/// # Design: Value Object
///
/// `MoveRecord` is a plain data struct (not a Component) because:
/// - Moves are stored in collections (Vec in MoveHistory resource)
/// - Multiple moves can reference the same piece
/// - Move records are immutable once created
///
/// # Fields
///
/// ## Piece Information
/// - `piece_type`: What kind of piece moved (Pawn, Knight, etc.)
/// - `piece_color`: Which player made the move (White or Black)
///
/// ## Position Information
/// - `from`: Starting square (x, y) coordinates
/// - `to`: Destination square (x, y) coordinates
///
/// ## Special Move Flags
/// - `captured`: Type of piece captured (if any)
/// - `is_castling`: King-rook special move (O-O or O-O-O)
/// - `is_en_passant`: Pawn captures pawn diagonally in passing
/// - `is_check`: Move puts opponent in check
/// - `is_checkmate`: Move ends the game (checkmate)
///
/// # Example
///
/// ```rust
/// use xfchess::game::components::MoveRecord;
/// use xfchess::rendering::pieces::{PieceType, PieceColor};
///
/// // Scholar's Mate final move: Qf7#
/// let checkmate_move = MoveRecord {
///     piece_type: PieceType::Queen,
///     piece_color: PieceColor::White,
///     from: (3, 7),  // d8 (assuming queen started there)
///     to: (5, 6),    // f7
///     captured: Some(PieceType::Pawn),  // Captures f7 pawn
///     is_castling: false,
///     is_en_passant: false,
///     is_check: true,
///     is_checkmate: true,
/// };
/// ```
#[derive(Clone, Copy, Debug, Reflect)]
pub struct MoveRecord {
    /// Type of piece that moved (Pawn, Rook, Knight, Bishop, Queen, King)
    pub piece_type: PieceType,

    /// Color of the piece that moved (White or Black)
    pub piece_color: PieceColor,

    /// Starting position (x, y) where x,y ∈ [0,7]
    ///
    /// Coordinates: (file, rank)
    /// - x=0 is file 'a', x=7 is file 'h'
    /// - y=0 is rank 1, y=7 is rank 8
    pub from: (u8, u8),

    /// Destination position (x, y) where x,y ∈ [0,7]
    pub to: (u8, u8),

    /// Type of piece captured during this move, if any
    ///
    /// - `None`: Move to empty square
    /// - `Some(PieceType)`: Capture move (piece removed from board)
    pub captured: Option<PieceType>,

    /// Whether this move was a castling maneuver
    ///
    /// Castling involves moving both king and rook:
    /// - Kingside (O-O): King moves 2 squares right
    /// - Queenside (O-O-O): King moves 2 squares left
    pub is_castling: bool,

    /// Whether this was an en passant capture
    ///
    /// Special pawn capture where:
    /// - Opponent's pawn just moved 2 squares forward
    /// - Your pawn captures it diagonally as if it moved only 1 square
    pub is_en_passant: bool,

    /// Whether this move puts the opponent's king in check
    ///
    /// Used for:
    /// - Displaying "+" in algebraic notation (e.g., "Qf7+")
    /// - Triggering check visual effects
    /// - Validating opponent must respond to check
    pub is_check: bool,

    /// Whether this move ends the game in checkmate
    ///
    /// Check AND no legal moves available to opponent.
    /// Displayed as "#" in notation (e.g., "Qf7#")
    pub is_checkmate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_phase_default() {
        //! Verifies GamePhase defaults to Setup

        let phase = GamePhase::default();
        assert_eq!(phase, GamePhase::Setup);
    }

    #[test]
    fn test_game_phase_transitions() {
        //! Tests all valid GamePhase values

        let phases = vec![
            GamePhase::Setup,
            GamePhase::Playing,
            GamePhase::Check,
            GamePhase::Checkmate,
            GamePhase::Stalemate,
        ];

        // All phases should be distinct
        for (i, phase1) in phases.iter().enumerate() {
            for (j, phase2) in phases.iter().enumerate() {
                if i == j {
                    assert_eq!(phase1, phase2);
                } else {
                    assert_ne!(phase1, phase2);
                }
            }
        }
    }

    #[test]
    fn test_game_phase_clone() {
        //! Tests GamePhase can be cloned

        let original = GamePhase::Check;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_game_phase_copy() {
        //! Verifies GamePhase implements Copy

        let original = GamePhase::Playing;
        let copied = original; // Copy, not move
        assert_eq!(original, copied);
        assert_eq!(original, GamePhase::Playing); // Still accessible
    }

    #[test]
    fn test_game_phase_debug() {
        //! Tests debug formatting is useful

        assert_eq!(format!("{:?}", GamePhase::Check), "Check");
        assert_eq!(format!("{:?}", GamePhase::Checkmate), "Checkmate");
    }

    #[test]
    fn test_game_phase_equality() {
        //! Tests PartialEq implementation

        assert_eq!(GamePhase::Playing, GamePhase::Playing);
        assert_ne!(GamePhase::Check, GamePhase::Checkmate);
    }

    #[test]
    fn test_move_record_creation() {
        //! Tests creating a basic move record

        let move_rec = MoveRecord {
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

        assert_eq!(move_rec.piece_type, PieceType::Pawn);
        assert_eq!(move_rec.from, (4, 1));
        assert_eq!(move_rec.to, (4, 3));
        assert!(move_rec.captured.is_none());
    }

    #[test]
    fn test_move_record_capture() {
        //! Tests move record with capture

        let capture_move = MoveRecord {
            piece_type: PieceType::Queen,
            piece_color: PieceColor::Black,
            from: (3, 7),
            to: (7, 3),
            captured: Some(PieceType::Rook),
            is_castling: false,
            is_en_passant: false,
            is_check: true,
            is_checkmate: false,
        };

        assert!(capture_move.captured.is_some());
        assert_eq!(capture_move.captured.unwrap(), PieceType::Rook);
        assert!(capture_move.is_check);
    }

    #[test]
    fn test_move_record_castling() {
        //! Tests castling move record

        let castling_move = MoveRecord {
            piece_type: PieceType::King,
            piece_color: PieceColor::White,
            from: (4, 0),
            to: (6, 0),
            captured: None,
            is_castling: true,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        assert!(castling_move.is_castling);
        assert_eq!(castling_move.piece_type, PieceType::King);
    }

    #[test]
    fn test_move_record_en_passant() {
        //! Tests en passant move record

        let en_passant = MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::Black,
            from: (4, 3),
            to: (3, 2),
            captured: Some(PieceType::Pawn),
            is_castling: false,
            is_en_passant: true,
            is_check: false,
            is_checkmate: false,
        };

        assert!(en_passant.is_en_passant);
        assert_eq!(en_passant.captured, Some(PieceType::Pawn));
    }

    #[test]
    fn test_move_record_checkmate() {
        //! Tests checkmate move record

        let checkmate = MoveRecord {
            piece_type: PieceType::Queen,
            piece_color: PieceColor::White,
            from: (3, 4),
            to: (5, 6),
            captured: Some(PieceType::Pawn),
            is_castling: false,
            is_en_passant: false,
            is_check: true,
            is_checkmate: true,
        };

        assert!(checkmate.is_checkmate);
        assert!(checkmate.is_check); // Checkmate implies check
    }

    #[test]
    fn test_move_record_clone() {
        //! Tests MoveRecord can be cloned

        let original = MoveRecord {
            piece_type: PieceType::Knight,
            piece_color: PieceColor::Black,
            from: (1, 0),
            to: (2, 2),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        let cloned = original.clone();
        assert_eq!(original.piece_type, cloned.piece_type);
        assert_eq!(original.from, cloned.from);
    }

    #[test]
    fn test_move_record_copy() {
        //! Tests MoveRecord implements Copy

        let original = MoveRecord {
            piece_type: PieceType::Bishop,
            piece_color: PieceColor::White,
            from: (2, 0),
            to: (5, 3),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        let copied = original; // Copy, not move
        assert_eq!(original.from, copied.from); // Original still accessible
    }
}
