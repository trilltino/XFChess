use crate::rendering::pieces::PieceType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKind {
    Normal,
    Capture,
    CastleKingside,
    CastleQueenside,
    EnPassant,
    Promote(PieceType),
}

#[derive(Debug, Clone, Copy)]
pub struct MoveStep {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub kind: MoveKind,
}
