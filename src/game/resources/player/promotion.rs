use crate::rendering::pieces::{PieceColor, PieceType};
use bevy::prelude::*;

#[derive(Resource, Default, Debug, Clone)]
pub struct PendingPromotion {
    pub pawn_entity: Option<Entity>,
    pub position: Option<(u8, u8)>,
    pub color: Option<PieceColor>,
    pub is_pending: bool,
}

impl PendingPromotion {
    pub fn start(&mut self, entity: Entity, position: (u8, u8), color: PieceColor) {
        self.pawn_entity = Some(entity);
        self.position = Some(position);
        self.color = Some(color);
        self.is_pending = true;
    }

    pub fn clear(&mut self) {
        self.pawn_entity = None;
        self.position = None;
        self.color = None;
        self.is_pending = false;
    }

    pub fn is_active(&self) -> bool {
        self.is_pending
    }
}

#[derive(bevy::ecs::message::Message, Debug, Clone)]
pub struct PromotionSelected {
    pub entity: Entity,
    pub position: (u8, u8),
    pub promoted_to: PieceType,
}

pub fn is_promotion_move(piece_type: PieceType, color: PieceColor, target_rank: u8) -> bool {
    if piece_type != PieceType::Pawn {
        return false;
    }
    match color {
        PieceColor::White => target_rank == 7, // White promotes on rank 8 (index 7)
        PieceColor::Black => target_rank == 0, // Black promotes on rank 1 (index 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_promotion_default() {
        let promo = PendingPromotion::default();

        assert!(promo.pawn_entity.is_none());
        assert!(promo.position.is_none());
        assert!(promo.color.is_none());
        assert!(!promo.is_pending);
        assert!(!promo.is_active());
    }

    #[test]
    fn test_pending_promotion_start() {
        let mut promo = PendingPromotion::default();

        promo.start(Entity::PLACEHOLDER, (4, 7), PieceColor::White);

        assert!(promo.pawn_entity.is_some());
        assert_eq!(promo.position, Some((4, 7)));
        assert_eq!(promo.color, Some(PieceColor::White));
        assert!(promo.is_pending);
        assert!(promo.is_active());
    }

    #[test]
    fn test_pending_promotion_clear() {
        let mut promo = PendingPromotion::default();
        promo.start(Entity::PLACEHOLDER, (4, 7), PieceColor::White);

        promo.clear();

        assert!(promo.pawn_entity.is_none());
        assert!(promo.position.is_none());
        assert!(promo.color.is_none());
        assert!(!promo.is_pending);
        assert!(!promo.is_active());
    }

    #[test]
    fn test_is_promotion_move_white() {
        assert!(is_promotion_move(PieceType::Pawn, PieceColor::White, 7));
        assert!(!is_promotion_move(PieceType::Pawn, PieceColor::White, 6));
        assert!(!is_promotion_move(PieceType::Pawn, PieceColor::White, 0));
    }

    #[test]
    fn test_is_promotion_move_black() {
        assert!(is_promotion_move(PieceType::Pawn, PieceColor::Black, 0));
        assert!(!is_promotion_move(PieceType::Pawn, PieceColor::Black, 1));
        assert!(!is_promotion_move(PieceType::Pawn, PieceColor::Black, 7));
    }

    #[test]
    fn test_is_promotion_move_non_pawn() {
        // Non-pawn pieces should never promote
        assert!(!is_promotion_move(PieceType::Queen, PieceColor::White, 7));
        assert!(!is_promotion_move(PieceType::Rook, PieceColor::Black, 0));
        assert!(!is_promotion_move(PieceType::Knight, PieceColor::White, 7));
    }
}
