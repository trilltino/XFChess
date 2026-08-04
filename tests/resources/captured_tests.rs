//! Integration tests for captured pieces resource extracted from doc tests
//! Original location: src/game/resources/history/captured.rs

use xfchess::game::resources::CapturedPieces;
use xfchess::rendering::pieces::{PieceColor, PieceType};

/// Test usage of CapturedPieces resource
/// Original: CapturedPieces struct-level example
#[test]
fn example_captured_pieces_usage() {
    let mut captured_pieces = CapturedPieces::default();

    // Add a capture
    captured_pieces.add_capture(PieceColor::Black, PieceType::Queen);

    // Check advantage
    let advantage = captured_pieces.material_advantage();
    assert_eq!(advantage, 9); // +9 for White
}

/// Test recording a capture
/// Original: add_capture method example
#[test]
fn example_add_capture() {
    let mut captured = CapturedPieces::default();

    // White captures Black's queen
    captured.add_capture(PieceColor::Black, PieceType::Queen);

    assert_eq!(captured.white_captured.len(), 1);
    assert_eq!(captured.white_captured[0], PieceType::Queen);
}
