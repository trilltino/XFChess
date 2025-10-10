//! Board state representation for move validation

use bevy::prelude::*;
use crate::rendering::pieces::{Piece, PieceColor};

/// Represents the state of the board for move validation
pub struct BoardState {
    pub pieces: Vec<(Entity, Piece, (u8, u8))>,
}

impl BoardState {
    pub fn is_empty(&self, pos: (u8, u8)) -> bool {
        !self.pieces.iter().any(|(_, _, p)| *p == pos)
    }

    pub fn get_piece_color(&self, pos: (u8, u8)) -> Option<PieceColor> {
        self.pieces
            .iter()
            .find(|(_, _, p)| *p == pos)
            .map(|(_, piece, _)| piece.color)
    }

    #[allow(dead_code)] // TODO: Will be used for complex move validation
    pub fn get_piece_at(&self, pos: (u8, u8)) -> Option<&Piece> {
        self.pieces
            .iter()
            .find(|(_, _, p)| *p == pos)
            .map(|(_, piece, _)| piece)
    }
}
