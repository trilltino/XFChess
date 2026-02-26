//! Integration tests for turn tracking resource extracted from doc tests
//! Original location: src/game/resources/turn/current.rs

use xfchess::game::resources::CurrentTurn;
use xfchess::rendering::pieces::PieceColor;

/// Test turn execution flow
/// Original: turn flow example
#[test]
fn example_execute_move_flow() {
    let mut current_turn = CurrentTurn::default();

    // White moves
    // ... execute move ...
    current_turn.switch();

    // Expect Black's turn
    assert_eq!(current_turn.color, PieceColor::Black);
}

/// Test switching turns
/// Original: switch method example
#[test]
fn example_switch_turns() {
    let mut turn = CurrentTurn::default();
    assert_eq!(turn.color, PieceColor::White);
    assert_eq!(turn.move_number, 1);

    turn.switch(); // Now Black's turn, still move 1
    assert_eq!(turn.color, PieceColor::Black);
    assert_eq!(turn.move_number, 1);

    turn.switch(); // Now White's turn, move 2
    assert_eq!(turn.color, PieceColor::White);
    assert_eq!(turn.move_number, 2);
}
