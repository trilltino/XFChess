use xfchess::game::resources::CapturedPieces;
use xfchess::rendering::pieces::{PieceColor, PieceType};

#[test]
fn example_captured_pieces_usage() {
    let mut captured_pieces = CapturedPieces::default();

    // Add a capture
    captured_pieces.add_capture(PieceColor::Black, PieceType::Queen);

    // Check advantage
    let advantage = captured_pieces.material_advantage();
    assert_eq!(advantage, 9); // +9 for White
}

#[test]
fn example_add_capture() {
    let mut captured = CapturedPieces::default();

    // White captures Black's queen
    captured.add_capture(PieceColor::Black, PieceType::Queen);

    assert_eq!(captured.white_captured.len(), 1);
    assert_eq!(captured.white_captured[0], PieceType::Queen);
}
