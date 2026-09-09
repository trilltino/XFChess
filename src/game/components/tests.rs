use super::*;
use crate::rendering::pieces::{PieceColor, PieceType};

// ============================================================================
// GamePhase Tests
// ============================================================================

#[test]
fn test_game_phase_default() {
    let phase = GamePhase::default();
    assert_eq!(phase, GamePhase::Setup, "GamePhase should default to Setup");
}

#[test]
fn test_game_phase_equality() {
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
    let record = MoveRecord {
        piece_type: PieceType::Knight,
        piece_color: PieceColor::White,
        from: (2, 2),
        to: (4, 3),
        captured: Some(PieceType::Pawn),
        is_castling: false,
        is_en_passant: false,
        is_check: true, // Knight capture could give check
        is_checkmate: false,
    };

    assert_eq!(record.captured, Some(PieceType::Pawn));
    assert!(record.is_check, "Capture should be able to give check");
}

#[test]
fn test_move_record_castling() {
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
    let has_moved = HasMoved::default();

    assert!(!has_moved.moved, "Pieces should not have moved initially");
    assert_eq!(has_moved.move_count, 0, "Move count should start at 0");
}

#[test]
fn test_has_moved_after_first_move() {
    let mut has_moved = HasMoved::default();

    has_moved.moved = true;
    has_moved.move_count = 1;

    assert!(has_moved.moved, "Piece should be marked as moved");
    assert_eq!(has_moved.move_count, 1, "Move count should be 1");
}

#[test]
fn test_has_moved_multiple_moves() {
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
    let original = HasMoved {
        moved: true,
        move_count: 2,
    };

    let copy = original; // This is a copy, not a move

    // Both should be usable
    assert_eq!(original.move_count, 2);
    assert_eq!(copy.move_count, 2);
}
