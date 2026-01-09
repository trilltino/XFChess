//! Pawn promotion resource for tracking pending promotions
//!
//! When a pawn reaches the opposite end of the board, this resource
//! stores the promotion state and allows the UI to prompt the player
//! for their choice of piece.

use crate::rendering::pieces::{PieceColor, PieceType};
use bevy::prelude::*;

/// Resource to track a pending pawn promotion
///
/// When a pawn reaches the 8th rank (for white) or 1st rank (for black),
/// this resource is populated with the promotion details. The UI displays
/// a selection dialog, and the game pauses until the player chooses.
#[derive(Resource, Default, Debug, Clone)]
pub struct PendingPromotion {
    /// The entity of the pawn being promoted
    pub pawn_entity: Option<Entity>,
    /// The position of the pawn (where it landed)
    pub position: Option<(u8, u8)>,
    /// The color of the pawn being promoted
    pub color: Option<PieceColor>,
    /// Whether a promotion is currently pending
    pub is_pending: bool,
}

impl PendingPromotion {
    /// Start a new promotion for a pawn
    pub fn start(&mut self, entity: Entity, position: (u8, u8), color: PieceColor) {
        self.pawn_entity = Some(entity);
        self.position = Some(position);
        self.color = Some(color);
        self.is_pending = true;
    }

    /// Clear the pending promotion (after player selects or cancels)
    pub fn clear(&mut self) {
        self.pawn_entity = None;
        self.position = None;
        self.color = None;
        self.is_pending = false;
    }

    /// Check if a promotion is pending
    pub fn is_active(&self) -> bool {
        self.is_pending
    }
}

/// Message sent when the player selects a promotion piece
#[derive(bevy::ecs::message::Message, Debug, Clone)]
pub struct PromotionSelected {
    pub entity: Entity,
    pub position: (u8, u8),
    pub color: PieceColor,
    pub promoted_to: PieceType,
}

/// Check if a pawn move results in promotion
pub fn is_promotion_move(piece_type: PieceType, color: PieceColor, target_rank: u8) -> bool {
    if piece_type != PieceType::Pawn {
        return false;
    }
    match color {
        PieceColor::White => target_rank == 7, // White promotes on rank 8 (index 7)
        PieceColor::Black => target_rank == 0, // Black promotes on rank 1 (index 0)
    }
}
