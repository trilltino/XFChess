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
    // The cache is empty until rebuilt; `default()` does not populate it.
    engine.rebuild_legal_move_cache();

    // Coords are (file, rank), 0-indexed — matching the rest of the codebase
    // (e2 = file 4, rank 1). White pawn on e2:
    let legal_moves = engine.get_legal_moves_for_square((4, 1), PieceColor::White);

    // Should include e3 (4, 2) and e4 (4, 3).
    assert!(
        legal_moves.contains(&(4, 2)),
        "Should be able to move to e3, got {legal_moves:?}"
    );
    assert!(
        legal_moves.contains(&(4, 3)),
        "Should be able to move to e4, got {legal_moves:?}"
    );
}
