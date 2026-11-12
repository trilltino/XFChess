use crate::{PositionKey, RulesError};
use shakmaty::{CastlingSide, Chess, Color, Position};

impl PositionKey {
    /// Standard-chess adapter. Shakmaty has already validated the position;
    /// `legal_ep_square` accounts for pins and discovered checks.
    pub fn from_shakmaty(position: &Chess) -> Result<Self, RulesError> {
        let mut squares = [0; 64];
        for (square, piece) in position.board() {
            squares[usize::from(square)] =
                piece.role as i8 * if piece.color.is_white() { 1 } else { -1 };
        }
        let mut castling = 0;
        for (colour, side, bit) in [
            (Color::White, CastlingSide::KingSide, 1),
            (Color::White, CastlingSide::QueenSide, 2),
            (Color::Black, CastlingSide::KingSide, 4),
            (Color::Black, CastlingSide::QueenSide, 8),
        ] {
            if position.castles().has(colour, side) {
                castling |= bit;
            }
        }
        Self::from_parts(
            &squares,
            castling,
            if position.turn().is_white() { 1 } else { -1 },
            position.legal_ep_square().map(u8::from),
        )
    }
}
