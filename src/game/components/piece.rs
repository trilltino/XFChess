//! Piece-related components

use bevy::prelude::*;

/// Component to track which piece is currently selected
/// TODO: Will be used when implementing visual selection indicators
#[allow(dead_code)]
#[derive(Component, Clone, Copy, Debug)]
pub struct SelectedPiece {
    pub entity: Entity,
    pub position: (u8, u8),
}

/// Component for pieces that have moved (for castling and pawn double-move)
/// TODO: Track piece movement for castling rules
#[allow(dead_code)]
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct HasMoved {
    pub moved: bool,
    pub move_count: u32,
}
