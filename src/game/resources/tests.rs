use super::*;
use crate::game::components::{GamePhase, MoveRecord};
use crate::rendering::pieces::PieceColor;

// ============================================================================
// CurrentTurn Tests
// ============================================================================

#[test]
fn test_current_turn_default() {
    let turn = CurrentTurn::default();

    assert_eq!(turn.color, PieceColor::White, "White should move first");
    assert_eq!(turn.move_number, 1, "Game should start at move 1");
}

#[test]
fn test_turn_switch_white_to_black() {
    let mut turn = CurrentTurn::default();
    turn.switch();

    assert_eq!(turn.color, PieceColor::Black, "Should switch to black");
    assert_eq!(
        turn.move_number, 1,
        "Move number should not increment when white moves"
    );
}

#[test]
fn test_turn_switch_black_to_white() {
    let mut turn = CurrentTurn {
        color: PieceColor::Black,
        move_number: 1,
    };
    turn.switch();

    assert_eq!(turn.color, PieceColor::White, "Should switch to white");
    assert_eq!(
        turn.move_number, 2,
        "Move number should increment when black completes their turn"
    );
}

#[test]
fn test_multiple_turn_switches() {
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
    let phase = CurrentGamePhase::default();
    assert_eq!(
        phase.0,
        GamePhase::Playing,
        "Game should start in Playing phase"
    );
}

#[test]
fn test_game_phase_transitions() {
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
    let history = MoveHistory::default();

    assert_eq!(history.len(), 0, "New history should have length 0");
    assert!(history.last_move().is_none(), "Should have no last move");
}

#[test]
fn test_move_history_add_move() {
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
    assert_eq!(
        last.captured,
        Some(crate::rendering::pieces::PieceType::Pawn),
        "Captured piece should be recorded"
    );
}

// ============================================================================
// GameTimer Tests
// ============================================================================

#[test]
fn test_game_timer_default() {
    let timer = GameTimer::default();

    assert_eq!(
        timer.white_time_left, 600.0,
        "White should start with 10 minutes"
    );
    assert_eq!(
        timer.black_time_left, 600.0,
        "Black should start with 10 minutes"
    );
    assert_eq!(timer.increment, 0.0, "Default should have no increment");
    assert!(!timer.is_running, "Timer should not be running initially");
}

#[test]
fn test_fischer_increment_white() {
    let mut timer = GameTimer {
        white_time_left: 100.0,
        black_time_left: 100.0,
        increment: 5.0,
        is_running: true,
    };

    timer.apply_increment(PieceColor::White);

    assert_eq!(timer.white_time_left, 105.0, "White should gain 5 seconds");
    assert_eq!(
        timer.black_time_left, 100.0,
        "Black time should be unchanged"
    );
}

#[test]
fn test_fischer_increment_black() {
    let mut timer = GameTimer {
        white_time_left: 100.0,
        black_time_left: 100.0,
        increment: 5.0,
        is_running: true,
    };

    timer.apply_increment(PieceColor::Black);

    assert_eq!(
        timer.white_time_left, 100.0,
        "White time should be unchanged"
    );
    assert_eq!(timer.black_time_left, 105.0, "Black should gain 5 seconds");
}

#[test]
fn test_fischer_increment_zero() {
    let mut timer = GameTimer {
        white_time_left: 100.0,
        black_time_left: 100.0,
        increment: 0.0,
        is_running: true,
    };

    timer.apply_increment(PieceColor::White);

    assert_eq!(
        timer.white_time_left, 100.0,
        "White time should be unchanged"
    );
    assert_eq!(
        timer.black_time_left, 100.0,
        "Black time should be unchanged"
    );
}

#[test]
fn test_multiple_increments() {
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
    let timer = GameTimer {
        white_time_left: 180.0, // 3 minutes
        black_time_left: 180.0,
        increment: 2.0,
        is_running: false,
    };

    assert_eq!(timer.white_time_left, 180.0);
    assert_eq!(timer.black_time_left, 180.0);
    assert_eq!(timer.increment, 2.0);
}
