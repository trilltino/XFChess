//! Integration tests for move history resource extracted from doc tests
//! Original location: src/game/resources/history/history.rs

use xfchess::game::components::MoveRecord;
use xfchess::game::resources::MoveHistory;
use xfchess::rendering::pieces::{PieceColor, PieceType};

/// Test recording moves
/// Original: recording moves example
#[test]
fn example_execute_move_system() {
    let mut history = MoveHistory::default();

    let move_record = MoveRecord {
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (4, 1),
        to: (4, 3),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };

    history.add_move(move_record);
    assert_eq!(history.len(), 1);
}

/// Test reviewing history
/// Original: reviewing history example
#[test]
fn example_display_last_move() {
    let mut history = MoveHistory::default();

    history.add_move(MoveRecord {
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (4, 1),
        to: (4, 3),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    });

    if let Some(last) = history.last_move() {
        assert_eq!(last.from, (4, 1));
        assert_eq!(last.to, (4, 3));
    } else {
        panic!("History should not be empty");
    }
}

/// Test adding a move to history
/// Original: add_move method example
#[test]
fn example_add_move_method() {
    let mut history = MoveHistory::default();

    history.add_move(MoveRecord {
        piece_type: PieceType::Knight,
        piece_color: PieceColor::Black,
        from: (1, 7),
        to: (2, 5),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: true, // Knight gives check
        is_checkmate: false,
    });

    assert_eq!(history.len(), 1);
    assert!(history.last_move().unwrap().is_check);
}

/// Test checking for en passant via last move
/// Original: last_move method example
#[test]
fn example_last_move_en_passant_check() {
    let mut history = MoveHistory::default();

    // White pawn moves two squares
    history.add_move(MoveRecord {
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (4, 1),
        to: (4, 3),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    });

    if let Some(last) = history.last_move() {
        if last.piece_type == PieceType::Pawn {
            let distance = (last.to.1 as i8 - last.from.1 as i8).abs();
            if distance == 2 {
                // En passant may be available
                assert!(true); // Logic verified
            } else {
                panic!("Should be distance 2");
            }
        }
    }
}

/// Test calculating full moves from ply
/// Original: len method example
#[test]
fn example_ply_count() {
    let mut history = MoveHistory::default();
    // Add dummy moves
    for _ in 0..3 {
        history.add_move(MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::White, // Simplified
            from: (0, 0),
            to: (0, 0),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        });
    }

    let ply_count = history.len();
    let full_moves = (ply_count / 2) + 1;

    assert_eq!(ply_count, 3);
    assert_eq!(full_moves, 2); // 3 ply = Move 2 for White
}

/// Test checking if history is empty
/// Original: is_empty method example
#[test]
fn example_is_empty() {
    let history = MoveHistory::default();
    if history.is_empty() {
        assert!(true);
    } else {
        panic!("Should be empty");
    }
}

/// Test clearing history
/// Original: clear method example
#[test]
fn example_clear_history() {
    let mut history = MoveHistory::default();
    history.add_move(MoveRecord {
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (0, 0),
        to: (0, 0),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    });

    history.clear();
    assert!(history.is_empty());
}

/// Test getting move by index
/// Original: get_move method example
#[test]
fn example_get_move() {
    let mut history = MoveHistory::default();
    // 1. e4
    history.add_move(MoveRecord {
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (4, 1),
        to: (4, 3),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    });

    if let Some(first_move) = history.get_move(0) {
        assert_eq!(first_move.piece_type, PieceType::Pawn);
        assert_eq!(first_move.to, (4, 3));
    } else {
        panic!("Should have first move");
    }
}

/// Test iterating over moves
/// Original: iter method example
#[test]
fn example_iter_moves() {
    let mut history = MoveHistory::default();
    history.add_move(MoveRecord {
        piece_type: PieceType::Pawn,
        piece_color: PieceColor::White,
        from: (0, 0),
        to: (0, 0),
        captured: None,
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    });

    let mut count = 0;
    for (i, _move_record) in history.iter().enumerate() {
        count += 1;
        assert_eq!(i, 0);
    }
    assert_eq!(count, 1);
}
