//! Move-related components

use bevy::prelude::*;
use crate::rendering::pieces::PieceColor;

/// Represents a possible move for a piece
/// TODO: Will be used for move animation and validation visualization
#[allow(dead_code)]
#[derive(Component, Clone, Copy, Debug)]
pub struct PossibleMove {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub is_capture: bool,
    pub is_castling: bool,
    pub is_en_passant: bool,
}

/// Marks a square as a valid move destination
/// TODO: Will be used for visual highlighting of valid moves
#[allow(dead_code)]
#[derive(Component, Clone, Copy, Debug)]
pub struct ValidMoveMarker;

/// Component marking a square as being attacked by a piece
/// TODO: Will be used for check detection and king safety
#[allow(dead_code)]
#[derive(Component, Clone, Copy, Debug)]
pub struct AttackedSquare {
    pub by_color: PieceColor,
}
