use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct PendingTurnAdvance {
    pending: std::collections::VecDeque<PendingTurn>,
}

#[derive(Clone, Copy, Debug, Reflect)]
pub struct PendingTurn {
    pub mover: PieceColor,
}

impl PendingTurnAdvance {
    pub fn request(&mut self, mover: PieceColor) -> bool {
        if !self.pending.is_empty() {
            return false;
        }
        self.record_accepted(mover);
        true
    }

    /// Each accepted move owes one turn flip and clock increment, including
    /// several moves received in the same frame. This is unrelated to motion.
    pub fn record_accepted(&mut self, mover: PieceColor) {
        self.pending.push_back(PendingTurn { mover });
    }

    pub fn take(&mut self) -> Option<PendingTurn> {
        self.pending.pop_front()
    }

    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_turn_request() -> Result<(), Box<dyn std::error::Error>> {
        let mut pending = PendingTurnAdvance::default();
        assert!(pending.request(PieceColor::White));
        assert!(pending.is_pending());
        assert!(!pending.request(PieceColor::Black));
        assert_eq!(
            pending.take().ok_or("missing test value")?.mover,
            PieceColor::White
        );
        assert!(!pending.is_pending());

        Ok(())
    }
}
