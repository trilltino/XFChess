//! Game state components

use bevy::prelude::*;
use crate::rendering::pieces::{PieceColor, PieceType};

/// Component for the current game phase
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)] // Check, Checkmate, Stalemate will be used when game logic is fully implemented
pub enum GamePhase {
    Setup,
    Playing,
    Check,
    Checkmate,
    Stalemate,
}

impl Default for GamePhase {
    fn default() -> Self {
        GamePhase::Setup
    }
}

/// Move record for history (not a component, used in MoveHistory resource)
#[allow(dead_code)] // All fields are used by MoveHistory but not individually accessed yet
#[derive(Clone, Copy, Debug)]
pub struct MoveRecord {
    pub piece_type: PieceType,
    pub piece_color: PieceColor,
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub captured: Option<PieceType>,
    pub is_castling: bool,
    pub is_en_passant: bool,
    pub is_check: bool,
    pub is_checkmate: bool,
}
