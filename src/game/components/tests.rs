//! Component module unit tests
//!
//! This test module validates the behavior of game components including:
//! - GamePhase state transitions
//! - MoveRecord data structure integrity
//! - HasMoved component state tracking
//!
//! Components are pure data structures without logic, so these tests primarily
//! validate that the types can be constructed correctly, have appropriate defaults,
//! and support the operations required by game systems.

use super::*;
use crate::rendering::pieces::{PieceColor, PieceType};

// ============================================================================
// GamePhase Tests
// ============================================================================

#[test]
fn test_game_phase_default() {
    //! Tests that GamePhase defaults to Setup state
    //!
    //! The Setup phase is the initial game state before pieces are placed.
    //! This test ensures the Default implementation returns the correct
    //! starting state for new game entities.

    let phase = GamePhase::default();
    assert_eq!(phase, GamePhase::Setup, "GamePhase should default to Setup");
}

#[test]
fn test_game_phase_equality() {
    //! Tests GamePhase equality comparisons
    //!
    //! Systems often need to check the current game phase to determine
    //! what actions are allowed. This test validates that PartialEq
    //! works correctly for all phase variants.

    assert_eq!(GamePhase::Setup, GamePhase::Setup);
    assert_eq!(GamePhase::Playing, GamePhase::Playing);
    assert_eq!(GamePhase::Check, GamePhase::Check);
    assert_eq!(GamePhase::Checkmate, GamePhase::Checkmate);
    assert_eq!(GamePhase::Stalemate, GamePhase::Stalemate);

    assert_ne!(GamePhase::Setup, GamePhase::Playing);
    assert_ne!(GamePhase::Check, GamePhase::Checkmate);
}

#[test]
fn test_game_phase_all_variants() {
    //! Tests that all GamePhase variants can be constructed
    //!
    //! This exhaustive test ensures all valid game states exist and can
    //! be created. It serves as documentation of all possible game phases
    //! and prevents accidental removal of variants during refactoring.

    let setup = GamePhase::Setup;
    let playing = GamePhase::Playing;
    let check = GamePhase::Check;
    let checkmate = GamePhase::Checkmate;
    let stalemate = GamePhase::Stalemate;

    // Ensure all phases are distinct
    assert_ne!(setup, playing);
    assert_ne!(playing, check);
    assert_ne!(check, checkmate);
    assert_ne!(checkmate, stalemate);
}

// ============================================================================
// MoveRecord Tests
// ============================================================================

#[test]
fn test_move_record_simple_move() {
    //! Tests creating a basic move record
    //!
    //! A MoveRecord captures all information about a move for history tracking.
    //! This test validates that a simple pawn forward move can be correctly
    //! represented with all required fields.

    let record = MoveRecord {
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (1, 4),
        to: (3, 4),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    assert_eq!(record.piece_type, PieceType::Pawn);
    assert_eq!(record.piece_color, PieceColor::White);
    assert_eq!(record.from, (1, 4));
    assert_eq!(record.to, (3, 4));
    assert_eq!(record.captured, None);
    assert!(!record.is_castling);
    assert!(!record.is_en_passant);
    assert!(!record.is_check);
    assert!(!record.is_checkmate);
}

#[test]
fn test_move_record_with_capture() {
    //! Tests creating a move record with a capture
    //!
    //! When a piece captures an opponent's piece, the captured piece type
    //! is stored in the MoveRecord. This is critical for move undo functionality
    //! and for generating algebraic notation (e.g., "Nxe5" for knight takes e5).

    let record = MoveRecord {
        piece_type: PieceType::Knight,
        piece_color: PieceColor::White,
        from: (2, 2),
        to: (4, 3),
        captured: Some(PieceType::Pawn),
        is_castling: false,
        is_en_passant: false,
        is_check: true,  // Knight capture could give check
        is_checkmate: false,
    };

    assert_eq!(record.captured, Some(PieceType::Pawn));
    assert!(record.is_check, "Capture should be able to give check");
}

#[test]
fn test_move_record_castling() {
    //! Tests creating a castling move record
    //!
    //! Castling is a special move involving the king and rook. The MoveRecord
    //! must flag this with is_castling=true so systems can handle the special
    //! logic of moving two pieces simultaneously.

    let kingside_castle = MoveRecord {
        piece_type: PieceType::King,
        piece_color: PieceColor::White,
        from: (0, 4),
        to: (0, 6),
        captured: None,
        is_castling: true,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    assert!(kingside_castle.is_castling);
    assert_eq!(kingside_castle.piece_type, PieceType::King);
    assert_eq!(kingside_castle.from, (0, 4), "King starts at e1");
    assert_eq!(kingside_castle.to, (0, 6), "King moves to g1");
}

#[test]
fn test_move_record_en_passant() {
    //! Tests creating an en passant capture move record
    //!
    //! En passant is a special pawn capture where the capturing pawn moves
    //! diagonally to an empty square and captures a pawn on an adjacent file.
    //! This flag allows systems to handle the unique capture logic.

    let en_passant_capture = MoveRecord {
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (4, 3),
        to: (5, 4),
        captured: Some(PieceType::Pawn),
        is_castling: false,
        is_en_passant: true,
        is_check: false,
        is_checkmate: false,
    };

    assert!(en_passant_capture.is_en_passant);
    assert_eq!(en_passant_capture.captured, Some(PieceType::Pawn));
}

#[test]
fn test_move_record_checkmate() {
    //! Tests creating a checkmating move record
    //!
    //! A checkmate move ends the game. The MoveRecord must flag both
    //! is_check and is_checkmate so the game state can transition to the
    //! appropriate end state and display victory conditions.

    let checkmate_move = MoveRecord {
        piece_type: PieceType::Queen,
        piece_color: PieceColor::White,
        from: (5, 3),
        to: (7, 5),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: true,
        is_checkmate: true,
    };

    assert!(checkmate_move.is_check);
    assert!(checkmate_move.is_checkmate);
    assert_eq!(checkmate_move.piece_type, PieceType::Queen);
}

#[test]
fn test_move_record_clone() {
    //! Tests that MoveRecord implements Clone correctly
    //!
    //! Move history requires cloning MoveRecords for storage.
    //! This test validates that all fields are correctly copied
    //! and the clone is independent of the original.

    let original = MoveRecord {
        piece_type: PieceType::Rook,
        piece_color: PieceColor::Black,
        from: (7, 0),
        to: (7, 7),
        captured: Some(PieceType::Bishop),
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    let cloned = original.clone();

    assert_eq!(cloned.piece_type, original.piece_type);
    assert_eq!(cloned.piece_color, original.piece_color);
    assert_eq!(cloned.from, original.from);
    assert_eq!(cloned.to, original.to);
    assert_eq!(cloned.captured, original.captured);
    assert_eq!(cloned.is_castling, original.is_castling);
    assert_eq!(cloned.is_en_passant, original.is_en_passant);
    assert_eq!(cloned.is_check, original.is_check);
    assert_eq!(cloned.is_checkmate, original.is_checkmate);
}

// ============================================================================
// HasMoved Component Tests
// ============================================================================

#[test]
fn test_has_moved_default() {
    //! Tests HasMoved component default state
    //!
    //! Pieces start the game not having moved (moved=false, move_count=0).
    //! This is important for castling rules (king and rooks can't have moved)
    //! and pawn double-move rules (only allowed on first move).

    let has_moved = HasMoved::default();

    assert!(!has_moved.moved, "Pieces should not have moved initially");
    assert_eq!(has_moved.move_count, 0, "Move count should start at 0");
}

#[test]
fn test_has_moved_after_first_move() {
    //! Tests HasMoved state after a piece moves
    //!
    //! When a piece makes its first move, the moved flag should be set to true
    //! and move_count should increment to 1. This prevents illegal castling
    //! and disallows pawn double-moves after the first move.

    let mut has_moved = HasMoved::default();

    has_moved.moved = true;
    has_moved.move_count = 1;

    assert!(has_moved.moved, "Piece should be marked as moved");
    assert_eq!(has_moved.move_count, 1, "Move count should be 1");
}

#[test]
fn test_has_moved_multiple_moves() {
    //! Tests HasMoved state tracking multiple moves
    //!
    //! The move_count field tracks how many times a piece has moved.
    //! While the moved boolean is sufficient for castling and pawn rules,
    //! move_count could be used for advanced features like showing piece
    //! activity heatmaps or detecting three-fold repetition.

    let mut has_moved = HasMoved {
        moved: true,
        move_count: 5,
    };

    has_moved.move_count += 1;

    assert_eq!(has_moved.move_count, 6, "Move count should increment");
    assert!(has_moved.moved, "Moved flag should remain true");
}

#[test]
fn test_has_moved_clone() {
    //! Tests HasMoved Clone implementation
    //!
    //! HasMoved may need to be cloned for undo functionality or when
    //! saving game state. This test validates that cloning preserves
    //! both the moved flag and move_count.

    let original = HasMoved {
        moved: true,
        move_count: 3,
    };

    let cloned = original;

    assert_eq!(cloned.moved, original.moved);
    assert_eq!(cloned.move_count, original.move_count);
}

#[test]
fn test_has_moved_copy_semantics() {
    //! Tests that HasMoved uses copy semantics
    //!
    //! HasMoved implements Copy, meaning assignments create independent copies
    //! rather than moves. This is important for efficient ECS queries where
    //! components are frequently read without requiring explicit clones.

    let original = HasMoved {
        moved: true,
        move_count: 2,
    };

    let copy = original; // This is a copy, not a move

    // Both should be usable
    assert_eq!(original.move_count, 2);
    assert_eq!(copy.move_count, 2);
}
