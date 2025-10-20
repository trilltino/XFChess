//! Resource module unit tests
//!
//! This test module validates the behavior of all game resources including:
//! - Turn tracking and move number incrementing
//! - Game phase transitions
//! - Move history management
//! - Fischer increment timer logic
//!
//! These are pure data structure tests that verify resource state management
//! without requiring the full ECS system. This allows quick, focused testing
//! of game state logic independently from Bevy's scheduling and system execution.

use super::*;
use crate::rendering::pieces::PieceColor;
use crate::game::components::{GamePhase, MoveRecord};

// ============================================================================
// CurrentTurn Tests
// ============================================================================

#[test]
fn test_current_turn_default() {
    //! Tests that CurrentTurn initializes correctly
    //!
    //! In chess, white always moves first. The default turn state should
    //! reflect this starting condition with move_number starting at 1.
    //! This ensures games begin in the correct state without manual setup.

    let turn = CurrentTurn::default();

    assert_eq!(turn.color, PieceColor::White, "White should move first");
    assert_eq!(turn.move_number, 1, "Game should start at move 1");
}

#[test]
fn test_turn_switch_white_to_black() {
    //! Tests switching from white's turn to black's turn
    //!
    //! When white completes a move, the turn switches to black but the
    //! move number should NOT increment yet. In chess notation, a "move"
    //! consists of both white and black's turns (e.g., "1. e4 e5" is move 1).
    //! The move number only increments when black completes their turn.

    let mut turn = CurrentTurn::default();
    turn.switch();

    assert_eq!(turn.color, PieceColor::Black, "Should switch to black");
    assert_eq!(turn.move_number, 1, "Move number should not increment when white moves");
}

#[test]
fn test_turn_switch_black_to_white() {
    //! Tests switching from black's turn back to white's turn
    //!
    //! When black completes a move, the turn switches back to white AND
    //! the move number increments. This test verifies the asymmetric behavior
    //! of move number incrementing (only on black->white transitions).

    let mut turn = CurrentTurn {
        color: PieceColor::Black,
        move_number: 1,
    };
    turn.switch();

    assert_eq!(turn.color, PieceColor::White, "Should switch to white");
    assert_eq!(turn.move_number, 2, "Move number should increment when black completes their turn");
}

#[test]
fn test_multiple_turn_switches() {
    //! Tests multiple consecutive turn switches
    //!
    //! Validates that the turn switching logic correctly alternates between
    //! players and increments move numbers at the right times over multiple
    //! turns. This ensures the state machine behaves correctly throughout
    //! a full game sequence.

    let mut turn = CurrentTurn::default();

    // Move 1: White to Black
    turn.switch();
    assert_eq!(turn.color, PieceColor::Black);
    assert_eq!(turn.move_number, 1);

    // Move 1 complete: Black to White (move 2 begins)
    turn.switch();
    assert_eq!(turn.color, PieceColor::White);
    assert_eq!(turn.move_number, 2);

    // Move 2: White to Black
    turn.switch();
    assert_eq!(turn.color, PieceColor::Black);
    assert_eq!(turn.move_number, 2);

    // Move 2 complete: Black to White (move 3 begins)
    turn.switch();
    assert_eq!(turn.color, PieceColor::White);
    assert_eq!(turn.move_number, 3);
}

// ============================================================================
// CurrentGamePhase Tests
// ============================================================================

#[test]
fn test_game_phase_default() {
    //! Tests that CurrentGamePhase initializes to Playing state
    //!
    //! Games start in the Playing phase. This default ensures that systems
    //! which check the game phase will behave correctly from the start without
    //! requiring explicit initialization code.

    let phase = CurrentGamePhase::default();
    assert_eq!(phase.0, GamePhase::Playing, "Game should start in Playing phase");
}

#[test]
fn test_game_phase_transitions() {
    //! Tests game phase state transitions
    //!
    //! Validates that the CurrentGamePhase resource can hold different
    //! game states (Playing, Checkmate, Stalemate). This is a simple
    //! data holder test, but ensures the type system allows all valid
    //! phase values.

    let mut phase = CurrentGamePhase::default();

    phase.0 = GamePhase::Setup;
    assert_eq!(phase.0, GamePhase::Setup);

    phase.0 = GamePhase::Check;
    assert_eq!(phase.0, GamePhase::Check);

    phase.0 = GamePhase::Checkmate;
    assert_eq!(phase.0, GamePhase::Checkmate);

    phase.0 = GamePhase::Stalemate;
    assert_eq!(phase.0, GamePhase::Stalemate);

    phase.0 = GamePhase::Playing;
    assert_eq!(phase.0, GamePhase::Playing);
}

// ============================================================================
// MoveHistory Tests
// ============================================================================

#[test]
fn test_move_history_default() {
    //! Tests that MoveHistory initializes as empty
    //!
    //! A new game should have no move history. This test validates that
    //! the default state is correct and all query methods return appropriate
    //! values for an empty history.

    let history = MoveHistory::default();

    assert_eq!(history.len(), 0, "New history should have length 0");
    assert!(history.last_move().is_none(), "Should have no last move");
}

#[test]
fn test_move_history_add_move() {
    //! Tests adding moves to the history
    //!
    //! Validates that the add_move method correctly appends moves to the
    //! internal vector and updates all derived state (length, emptiness, etc.).
    //! This is the core functionality for tracking game progression.

    let mut history = MoveHistory::default();

    let move1 = MoveRecord {
        from: (1, 4),
        to: (3, 4),
        piece_type: crate::rendering::pieces::PieceType::Pawn,
        piece_color: PieceColor::White,
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    history.add_move(move1);

    assert_eq!(history.len(), 1, "Should have 1 move");

    let last = history.last_move().expect("Should have a last move");
    assert_eq!(last.from, (1, 4));
    assert_eq!(last.to, (3, 4));
}

#[test]
fn test_move_history_multiple_moves() {
    //! Tests adding multiple moves to the history
    //!
    //! Validates that the history correctly maintains move order and that
    //! last_move() always returns the most recent move. This is critical
    //! for features like undo, move replay, and game notation export.

    let mut history = MoveHistory::default();

    let move1 = MoveRecord {
        from: (1, 4),
        to: (3, 4),
        piece_type: crate::rendering::pieces::PieceType::Pawn,
        piece_color: PieceColor::White,
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    let move2 = MoveRecord {
        from: (6, 4),
        to: (4, 4),
        piece_type: crate::rendering::pieces::PieceType::Pawn,
        piece_color: PieceColor::Black,
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    let move3 = MoveRecord {
        from: (0, 1),
        to: (2, 2),
        piece_type: crate::rendering::pieces::PieceType::Knight,
        piece_color: PieceColor::White,
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    history.add_move(move1);
    history.add_move(move2);
    history.add_move(move3);

    assert_eq!(history.len(), 3, "Should have 3 moves");

    let last = history.last_move().expect("Should have a last move");
    assert_eq!(last.from, (0, 1), "Last move should be the knight move");
    assert_eq!(last.to, (2, 2));
}

#[test]
fn test_move_history_with_capture() {
    //! Tests recording a move with a capture
    //!
    //! Validates that the history correctly stores captured piece information.
    //! This is important for move undo functionality (to restore captured pieces)
    //! and for game notation generation (captures are notated differently).

    let mut history = MoveHistory::default();

    let move_with_capture = MoveRecord {
        from: (3, 4),
        to: (4, 5),
        piece_type: crate::rendering::pieces::PieceType::Pawn,
        piece_color: PieceColor::White,
        captured: Some(crate::rendering::pieces::PieceType::Pawn),
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    history.add_move(move_with_capture);

    let last = history.last_move().expect("Should have a last move");
    assert_eq!(last.captured, Some(crate::rendering::pieces::PieceType::Pawn),
        "Captured piece should be recorded");
}

// ============================================================================
// GameTimer Tests
// ============================================================================

#[test]
fn test_game_timer_default() {
    //! Tests that GameTimer initializes with standard 10-minute time control
    //!
    //! The default timer configuration is 10 minutes per player with no
    //! Fischer increment and the timer not running. This is a common
    //! casual time control and serves as a sensible default for new games.

    let timer = GameTimer::default();

    assert_eq!(timer.white_time_left, 600.0, "White should start with 10 minutes");
    assert_eq!(timer.black_time_left, 600.0, "Black should start with 10 minutes");
    assert_eq!(timer.increment, 0.0, "Default should have no increment");
    assert!(!timer.is_running, "Timer should not be running initially");
}

#[test]
fn test_fischer_increment_white() {
    //! Tests Fischer increment application for white player
    //!
    //! Fischer increment adds time to the player who just moved. This prevents
    //! games from always ending in time scrambles and rewards fast play.
    //! When white completes a move with a 5-second increment, they should
    //! receive 5 additional seconds.

    let mut timer = GameTimer {
        white_time_left: 100.0,
        black_time_left: 100.0,
        increment: 5.0,
        is_running: true,
    };

    timer.apply_increment(PieceColor::White);

    assert_eq!(timer.white_time_left, 105.0, "White should gain 5 seconds");
    assert_eq!(timer.black_time_left, 100.0, "Black time should be unchanged");
}

#[test]
fn test_fischer_increment_black() {
    //! Tests Fischer increment application for black player
    //!
    //! Same as the white increment test, but validates the symmetric behavior
    //! for the black player. Ensures the color matching logic works correctly
    //! in both branches.

    let mut timer = GameTimer {
        white_time_left: 100.0,
        black_time_left: 100.0,
        increment: 5.0,
        is_running: true,
    };

    timer.apply_increment(PieceColor::Black);

    assert_eq!(timer.white_time_left, 100.0, "White time should be unchanged");
    assert_eq!(timer.black_time_left, 105.0, "Black should gain 5 seconds");
}

#[test]
fn test_fischer_increment_zero() {
    //! Tests that zero increment does not modify times
    //!
    //! When increment is set to 0 (sudden death time control), applying
    //! the increment should be a no-op. This test ensures we don't add
    //! 0.0 or cause floating-point issues when increment is disabled.

    let mut timer = GameTimer {
        white_time_left: 100.0,
        black_time_left: 100.0,
        increment: 0.0,
        is_running: true,
    };

    timer.apply_increment(PieceColor::White);

    assert_eq!(timer.white_time_left, 100.0, "White time should be unchanged");
    assert_eq!(timer.black_time_left, 100.0, "Black time should be unchanged");
}

#[test]
fn test_multiple_increments() {
    //! Tests multiple consecutive increment applications
    //!
    //! Simulates a sequence of moves where both players receive increments.
    //! Validates that increments stack correctly and don't interfere with
    //! each other. This is important for ensuring timer accuracy over long games.

    let mut timer = GameTimer {
        white_time_left: 100.0,
        black_time_left: 100.0,
        increment: 3.0,
        is_running: true,
    };

    // White moves
    timer.apply_increment(PieceColor::White);
    assert_eq!(timer.white_time_left, 103.0);

    // Black moves
    timer.apply_increment(PieceColor::Black);
    assert_eq!(timer.black_time_left, 103.0);

    // White moves again
    timer.apply_increment(PieceColor::White);
    assert_eq!(timer.white_time_left, 106.0);

    // Black moves again
    timer.apply_increment(PieceColor::Black);
    assert_eq!(timer.black_time_left, 106.0);
}

#[test]
fn test_custom_time_control() {
    //! Tests creating a timer with custom time control settings
    //!
    //! Validates that the GameTimer can be constructed with arbitrary
    //! time controls. This test uses a 3-minute blitz game with 2-second
    //! increment (3+2), a common online chess format.

    let timer = GameTimer {
        white_time_left: 180.0,  // 3 minutes
        black_time_left: 180.0,
        increment: 2.0,
        is_running: false,
    };

    assert_eq!(timer.white_time_left, 180.0);
    assert_eq!(timer.black_time_left, 180.0);
    assert_eq!(timer.increment, 2.0);
}
