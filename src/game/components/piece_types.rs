use bevy::prelude::*;

#[derive(Clone, Copy, Debug, Component, PartialEq, Eq, Hash, Reflect, Default)]
#[reflect(Component)]
pub enum PieceColor {
    #[default]
    White,
    Black,
}

#[derive(
    Component,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Debug,
    Reflect,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Component)]
pub enum PieceType {
    #[default]
    King,
    Queen,
    Bishop,
    Knight,
    Rook,
    Pawn,
}

impl PieceType {
    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            'q' => Some(PieceType::Queen),
            'b' => Some(PieceType::Bishop),
            'n' => Some(PieceType::Knight),
            'r' => Some(PieceType::Rook),
            _ => None,
        }
    }
}

#[derive(Component, Clone, Debug, Copy, Reflect)]
#[reflect(Component)]
pub struct Piece {
    pub color: PieceColor,
    pub piece_type: PieceType,
    pub x: u8,
    pub y: u8,
}

impl Piece {
    pub fn new(color: PieceColor, piece_type: PieceType, file: u8, rank: u8) -> Self {
        Self {
            color,
            piece_type,
            x: file,
            y: rank,
        }
    }
}
