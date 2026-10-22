use anyhow::Context;
use xfchess_app::game::components::MoveRecord;
use xfchess_app::game::resources::MoveHistory;
use xfchess_app::rendering::pieces::{PieceColor, PieceType};

#[test]
fn example_execute_move_system() {
    let mut history = MoveHistory::default();

    let move_record = MoveRecord {
        promotion: None,
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

#[test]
fn example_display_last_move() {
    let mut history = MoveHistory::default();

    history.add_move(MoveRecord {
        promotion: None,
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

    assert_eq!(
        history.last_move().map(|last| (last.from, last.to)),
        Some(((4, 1), (4, 3)))
    );
}

#[test]
fn example_add_move_method() -> anyhow::Result<()> {
    let mut history = MoveHistory::default();

    history.add_move(MoveRecord {
        promotion: None,
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
    assert!(
        history
            .last_move()
            .context(concat!(file!(), ":", line!()))?
            .is_check
    );
    Ok(())
}

#[test]
fn example_last_move_en_passant_check() {
    let mut history = MoveHistory::default();

    // White pawn moves two squares
    history.add_move(MoveRecord {
        promotion: None,
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

    assert_eq!(
        history
            .last_move()
            .map(|last| (last.piece_type, last.to.1.abs_diff(last.from.1))),
        Some((PieceType::Pawn, 2))
    );
}

#[test]
fn example_ply_count() {
    let mut history = MoveHistory::default();
    for _ in 0..3 {
        history.add_move(MoveRecord {
            promotion: None,
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

#[test]
fn example_is_empty() {
    let history = MoveHistory::default();
    assert!(history.is_empty());
}

#[test]
fn example_clear_history() {
    let mut history = MoveHistory::default();
    history.add_move(MoveRecord {
        promotion: None,
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

#[test]
fn example_get_move() {
    let mut history = MoveHistory::default();
    history.add_move(MoveRecord {
        promotion: None,
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

#[test]
fn example_iter_moves() {
    let mut history = MoveHistory::default();
    history.add_move(MoveRecord {
        promotion: None,
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
