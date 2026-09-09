use crate::rendering::pieces::{PieceColor, PieceType};
use bevy::prelude::*;

#[derive(Resource, Default, Debug, Reflect)]
#[reflect(Resource)]
pub struct CapturedPieces {
    pub white_captured: Vec<PieceType>,
    pub black_captured: Vec<PieceType>,
}

impl CapturedPieces {
    pub fn add_capture(&mut self, captured_piece_color: PieceColor, piece_type: PieceType) {
        match captured_piece_color {
            // If white piece was captured, black gets credit
            PieceColor::White => self.black_captured.push(piece_type),
            // If black piece was captured, white gets credit
            PieceColor::Black => self.white_captured.push(piece_type),
        }
    }

    pub fn material_advantage(&self) -> i32 {
        let white_score: i32 = self.white_captured.iter().map(|p| piece_value(*p)).sum();
        let black_score: i32 = self.black_captured.iter().map(|p| piece_value(*p)).sum();
        white_score - black_score
    }

    pub fn clear(&mut self) {
        self.white_captured.clear();
        self.black_captured.clear();
    }
}

pub fn piece_value(piece_type: PieceType) -> i32 {
    match piece_type {
        PieceType::Pawn => 1,
        PieceType::Knight => 3,
        PieceType::Bishop => 3,
        PieceType::Rook => 5,
        PieceType::Queen => 9,
        PieceType::King => 0, // King doesn't have material value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_captured_pieces_default() {
        let captured = CapturedPieces::default();
        assert!(captured.white_captured.is_empty());
        assert!(captured.black_captured.is_empty());
        assert_eq!(captured.material_advantage(), 0);
    }

    #[test]
    fn test_add_capture_white_takes_black() {
        let mut captured = CapturedPieces::default();
        captured.add_capture(PieceColor::Black, PieceType::Queen);

        assert_eq!(captured.white_captured.len(), 1);
        assert_eq!(captured.white_captured[0], PieceType::Queen);
        assert_eq!(captured.black_captured.len(), 0);
    }

    #[test]
    fn test_add_capture_black_takes_white() {
        let mut captured = CapturedPieces::default();
        captured.add_capture(PieceColor::White, PieceType::Rook);

        assert_eq!(captured.black_captured.len(), 1);
        assert_eq!(captured.black_captured[0], PieceType::Rook);
        assert_eq!(captured.white_captured.len(), 0);
    }

    #[test]
    fn test_material_advantage_white_ahead() {
        let mut captured = CapturedPieces::default();
        captured.add_capture(PieceColor::Black, PieceType::Queen); // White takes Queen (9)
        captured.add_capture(PieceColor::White, PieceType::Pawn); // Black takes Pawn (1)

        assert_eq!(captured.material_advantage(), 8, "White should be +8 (9-1)");
    }

    #[test]
    fn test_material_advantage_black_ahead() {
        let mut captured = CapturedPieces::default();
        captured.add_capture(PieceColor::White, PieceType::Rook); // Black takes Rook (5)
        captured.add_capture(PieceColor::Black, PieceType::Bishop); // White takes Bishop (3)

        assert_eq!(
            captured.material_advantage(),
            -2,
            "Black should be +2 (3-5 = -2)"
        );
    }

    #[test]
    fn test_material_advantage_equal() {
        let mut captured = CapturedPieces::default();
        captured.add_capture(PieceColor::Black, PieceType::Knight); // White takes Knight (3)
        captured.add_capture(PieceColor::White, PieceType::Bishop); // Black takes Bishop (3)

        assert_eq!(captured.material_advantage(), 0, "Material should be equal");
    }

    #[test]
    fn test_piece_values() {
        assert_eq!(piece_value(PieceType::Pawn), 1);
        assert_eq!(piece_value(PieceType::Knight), 3);
        assert_eq!(piece_value(PieceType::Bishop), 3);
        assert_eq!(piece_value(PieceType::Rook), 5);
        assert_eq!(piece_value(PieceType::Queen), 9);
        assert_eq!(piece_value(PieceType::King), 0);
    }

    #[test]
    fn test_complex_exchange() {
        let mut captured = CapturedPieces::default();

        // White captures: Queen(9), Rook(5), Pawn(1) = 15
        captured.add_capture(PieceColor::Black, PieceType::Queen);
        captured.add_capture(PieceColor::Black, PieceType::Rook);
        captured.add_capture(PieceColor::Black, PieceType::Pawn);

        // Black captures: Rook(5), Knight(3), Bishop(3) = 11
        captured.add_capture(PieceColor::White, PieceType::Rook);
        captured.add_capture(PieceColor::White, PieceType::Knight);
        captured.add_capture(PieceColor::White, PieceType::Bishop);

        assert_eq!(captured.white_captured.len(), 3);
        assert_eq!(captured.black_captured.len(), 3);
        assert_eq!(
            captured.material_advantage(),
            4,
            "White should be +4 (15-11)"
        );
    }

    #[test]
    fn test_clear() {
        let mut captured = CapturedPieces::default();
        captured.add_capture(PieceColor::Black, PieceType::Queen);
        captured.add_capture(PieceColor::White, PieceType::Rook);

        captured.clear();

        assert!(captured.white_captured.is_empty());
        assert!(captured.black_captured.is_empty());
        assert_eq!(captured.material_advantage(), 0);
    }
}
