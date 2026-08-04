//! Integration tests for game state components extracted from doc tests
//!
//! These tests demonstrate usage patterns for GamePhase and MoveRecord.
//! Original location: src/game/components/game_state.rs module docs

use xfchess::game::components::{GamePhase, MoveRecord};
use xfchess::rendering::pieces::{PieceColor, PieceType};

/// Test demonstrating how to check game phase in a system
/// Original: game_state.rs module-level example
#[test]
fn example_display_game_status() {
    let phases = vec![
        (GamePhase::Check, "Check!"),
        (GamePhase::Checkmate, "Checkmate!"),
        (GamePhase::Stalemate, "Draw by stalemate"),
    ];

    for (phase, expected_msg) in phases {
        let message = match phase {
            GamePhase::Check => "Check!",
            GamePhase::Checkmate => "Checkmate!",
            GamePhase::Stalemate => "Draw by stalemate",
            _ => "",
        };
        assert_eq!(message, expected_msg);
    }
}

/// Test demonstrating how to create a move record
/// Original: game_state.rs module-level example
#[test]
fn example_recording_a_move() {
    let move_record = MoveRecord {
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (4, 1), // e2
        to: (4, 3),   // e4
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    assert_eq!(move_record.piece_type, PieceType::Pawn);
    assert_eq!(move_record.from, (4, 1));
    assert_eq!(move_record.to, (4, 3));
    assert!(move_record.captured.is_none());
}

/// Test demonstrating checking if moves are allowed
/// Original: GamePhase struct-level example
#[test]
fn example_allow_moves_system() {
    fn allow_moves(phase: GamePhase) -> bool {
        matches!(phase, GamePhase::Playing | GamePhase::Check)
    }

    assert!(allow_moves(GamePhase::Playing));
    assert!(allow_moves(GamePhase::Check));
    assert!(!allow_moves(GamePhase::Checkmate));
    assert!(!allow_moves(GamePhase::Stalemate));
    assert!(!allow_moves(GamePhase::Setup));
}

/// Test demonstrating Scholar's Mate checkmate move
/// Original: MoveRecord struct-level example
#[test]
fn example_scholars_mate_checkmate() {
    // Scholar's Mate final move: Qf7#
    let checkmate_move = MoveRecord {
        piece_type: PieceType::Queen,
        piece_color: PieceColor::White,
        from: (3, 7),                    // d8 (assuming queen started there)
        to: (5, 6),                      // f7
        captured: Some(PieceType::Pawn), // Captures f7 pawn
        is_castling: false,
        is_en_passant: false,
        is_check: true,
        is_checkmate: true,
    };

    assert_eq!(checkmate_move.piece_type, PieceType::Queen);
    assert!(checkmate_move.is_check);
    assert!(checkmate_move.is_checkmate);
    assert_eq!(checkmate_move.captured, Some(PieceType::Pawn));
}
