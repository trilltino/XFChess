//! Integration tests for chess engine resource extracted from doc tests
//! Original location: src/game/resources/engine/engine.rs

use xfchess::engine::board_state::ChessEngine;
use xfchess::rendering::pieces::PieceColor;
// Note: Direct dependency on chess_engine might be required for is_legal_move
// If this fails to compile, we might need to add chess_engine as dev-dependency
// or test via public API only.

/// Test move validation (Conceptual)
/// Original: validate_move example
/// This test verifies that the engine resource is accessible and we can use its helper methods.
#[test]
fn example_validate_move_helpers() {
    let from = (4, 1); // e2
    let to = (4, 3); // e4

    // Verify helper methods work as expected in the example context
    let src = ChessEngine::square_to_index(from.0, from.1);
    let dst = ChessEngine::square_to_index(to.0, to.1);

    assert_eq!(src, 12);
    assert_eq!(dst, 28);

    let engine = ChessEngine::default();
    assert_eq!(engine.current_turn, PieceColor::White);
    assert!(engine.fen.starts_with("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR"));
}

/// Test getting legal moves for a square
/// Original: get_legal_moves_for_square method example
#[test]
fn example_get_legal_moves_for_square() {
    let mut engine = ChessEngine::default();

    // White pawn on e2.
    // Rank 1 (0-indexed), File 4 (0-indexed implies e).
    // Coords: (1, 4)
    let legal_moves = engine.get_legal_moves_for_square((1, 4), PieceColor::White);

    // Should include e3 (2, 4) and e4 (3, 4)
    assert!(
        legal_moves.contains(&(2, 4)),
        "Should be able to move to e3"
    );
    assert!(
        legal_moves.contains(&(3, 4)),
        "Should be able to move to e4"
    );
}
