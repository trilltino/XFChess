use bevy::prelude::*;

use crate::rendering::pieces::{PieceColor, PieceType};

#[derive(Component, Debug)]
pub struct MiniPiece {
    pub file: u8,
    pub rank: u8,
    pub color: PieceColor,
    pub kind: PieceType,
}
