use crate::game::components::MoveRecord;
use bevy::prelude::*;

#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct MoveHistory {
    pub moves: Vec<MoveRecord>,

    pub sans: Vec<String>,
}

impl MoveHistory {
    pub fn add_move(&mut self, record: MoveRecord) {
        self.moves.push(record);
    }

    pub fn add_move_with_san(&mut self, record: MoveRecord, san: String) {
        self.moves.push(record);
        self.sans.push(san);
    }

    pub fn san_at(&self, index: usize) -> Option<&str> {
        self.sans.get(index).map(String::as_str)
    }

    pub fn last_move(&self) -> Option<&MoveRecord> {
        self.moves.last()
    }

    pub fn len(&self) -> usize {
        self.moves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    pub fn clear(&mut self) {
        self.moves.clear();
        self.sans.clear();
    }

    pub fn get_move(&self, index: usize) -> Option<&MoveRecord> {
        self.moves.get(index)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, MoveRecord> {
        self.moves.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, MoveRecord> {
        self.moves.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::pieces::{PieceColor, PieceType};

    #[test]
    fn test_move_history_default() {
        let history = MoveHistory::default();

        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.last_move().is_none());
    }

    #[test]
    fn test_add_move() {
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
        assert!(!history.is_empty());
        assert!(history.last_move().is_some());
    }

    #[test]
    fn test_last_move_returns_correct_move() {
        let mut history = MoveHistory::default();

        let first_move = MoveRecord {
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

        let second_move = MoveRecord {
            piece_type: PieceType::Knight,
            piece_color: PieceColor::Black,
            from: (1, 7),
            to: (2, 5),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        history.add_move(first_move);
        history.add_move(second_move);

        let last = history.last_move().unwrap();
        assert_eq!(last.piece_type, PieceType::Knight);
        assert_eq!(last.piece_color, PieceColor::Black);
    }

    #[test]
    fn test_len_increments_correctly() {
        let mut history = MoveHistory::default();

        assert_eq!(history.len(), 0);

        for i in 1..=10 {
            history.add_move(MoveRecord {
                piece_type: PieceType::Pawn,
                piece_color: if i % 2 == 1 {
                    PieceColor::White
                } else {
                    PieceColor::Black
                },
                from: (i as u8 % 8, 1),
                to: (i as u8 % 8, 3),
                captured: None,
                is_castling: false,
                is_en_passant: false,
                is_check: false,
                is_checkmate: false,
            });

            assert_eq!(history.len(), i);
        }
    }

    #[test]
    fn test_clear_removes_all_moves() {
        let mut history = MoveHistory::default();

        // Add several moves
        for _ in 0..5 {
            history.add_move(MoveRecord {
                piece_type: PieceType::Pawn,
                piece_color: PieceColor::White,
                from: (0, 1),
                to: (0, 3),
                captured: None,
                is_castling: false,
                is_en_passant: false,
                is_check: false,
                is_checkmate: false,
            });
        }

        assert_eq!(history.len(), 5);

        history.clear();

        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.last_move().is_none());
    }

    #[test]
    fn test_get_move_by_index() {
        let mut history = MoveHistory::default();

        let move1 = MoveRecord {
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

        let move2 = MoveRecord {
            piece_type: PieceType::Knight,
            piece_color: PieceColor::Black,
            from: (1, 7),
            to: (2, 5),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        history.add_move(move1);
        history.add_move(move2);

        // Check first move
        let retrieved_move1 = history.get_move(0).unwrap();
        assert_eq!(retrieved_move1.piece_type, PieceType::Pawn);
        assert_eq!(retrieved_move1.from, (4, 1));

        // Check second move
        let retrieved_move2 = history.get_move(1).unwrap();
        assert_eq!(retrieved_move2.piece_type, PieceType::Knight);
        assert_eq!(retrieved_move2.from, (1, 7));

        // Check out of bounds
        assert!(history.get_move(2).is_none());
    }

    #[test]
    fn test_iter_returns_all_moves() {
        let mut history = MoveHistory::default();

        // Add 3 moves
        for i in 0..3 {
            history.add_move(MoveRecord {
                piece_type: PieceType::Pawn,
                piece_color: if i % 2 == 0 {
                    PieceColor::White
                } else {
                    PieceColor::Black
                },
                from: (i, 1),
                to: (i, 3),
                captured: None,
                is_castling: false,
                is_en_passant: false,
                is_check: false,
                is_checkmate: false,
            });
        }

        let mut count = 0;
        for (i, move_record) in history.iter().enumerate() {
            assert_eq!(move_record.from.0, i as u8);
            count += 1;
        }

        assert_eq!(count, 3);
    }

    #[test]
    fn test_move_history_with_captures() {
        let mut history = MoveHistory::default();

        let capture_move = MoveRecord {
            piece_type: PieceType::Queen,
            piece_color: PieceColor::White,
            from: (3, 4),
            to: (7, 4),
            captured: Some(PieceType::Rook),
            is_castling: false,
            is_en_passant: false,
            is_check: true,
            is_checkmate: false,
        };

        history.add_move(capture_move);

        let last = history.last_move().unwrap();
        assert_eq!(last.captured, Some(PieceType::Rook));
        assert!(last.is_check);
    }

    #[test]
    fn test_move_history_with_special_moves() {
        let mut history = MoveHistory::default();

        // Castling
        history.add_move(MoveRecord {
            piece_type: PieceType::King,
            piece_color: PieceColor::White,
            from: (0, 4),
            to: (0, 6),
            captured: None,
            is_castling: true,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        });

        // En passant
        history.add_move(MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::Black,
            from: (4, 3),
            to: (3, 2),
            captured: Some(PieceType::Pawn),
            is_castling: false,
            is_en_passant: true,
            is_check: false,
            is_checkmate: false,
        });

        // Checkmate
        history.add_move(MoveRecord {
            piece_type: PieceType::Queen,
            piece_color: PieceColor::White,
            from: (3, 4),
            to: (5, 6),
            captured: Some(PieceType::Pawn),
            is_castling: false,
            is_en_passant: false,
            is_check: true,
            is_checkmate: true,
        });

        assert_eq!(history.len(), 3);
        assert!(history.get_move(0).unwrap().is_castling);
        assert!(history.get_move(1).unwrap().is_en_passant);
        assert!(history.get_move(2).unwrap().is_checkmate);
    }

    #[test]
    fn test_realistic_game_opening() {
        let mut history = MoveHistory::default();

        // 1. e4
        history.add_move(MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::White,
            from: (1, 4),
            to: (3, 4),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        });

        // 1... e5
        history.add_move(MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::Black,
            from: (6, 4),
            to: (4, 4),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        });

        // 2. Nf3
        history.add_move(MoveRecord {
            piece_type: PieceType::Knight,
            piece_color: PieceColor::White,
            from: (0, 6),
            to: (2, 5),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        });

        assert_eq!(history.len(), 3, "Should have recorded 3 half-moves");

        let first_move = history.get_move(0).unwrap();
        assert_eq!(first_move.piece_type, PieceType::Pawn);
        assert_eq!(first_move.piece_color, PieceColor::White);
        assert_eq!(first_move.from, (1, 4));
        assert_eq!(first_move.to, (3, 4));
    }
}
