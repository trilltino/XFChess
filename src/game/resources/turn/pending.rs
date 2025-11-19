use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct PendingTurnAdvance {
    pending: Option<PendingTurn>,
}

#[derive(Clone, Copy, Debug, Reflect)]
pub struct PendingTurn {
    pub mover: PieceColor,
}

impl PendingTurnAdvance {
    pub fn request(&mut self, mover: PieceColor) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.pending = Some(PendingTurn { mover });
        true
    }

    pub fn take(&mut self) -> Option<PendingTurn> {
        self.pending.take()
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_turn_request() {
        let mut pending = PendingTurnAdvance::default();
        assert!(pending.request(PieceColor::White));
        assert!(pending.is_pending());
        assert!(!pending.request(PieceColor::Black));
        assert_eq!(pending.take().unwrap().mover, PieceColor::White);
        assert!(!pending.is_pending());
    }
}
