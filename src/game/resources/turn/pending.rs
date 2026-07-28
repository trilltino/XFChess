//! A one-slot queue that decouples "a move was made" from "the turn advanced,"
//! so exactly one system applies the turn change even if multiple systems
//! could otherwise race to request it.

use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

/// Holds at most one pending turn advance at a time.
#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct PendingTurnAdvance {
    pending: Option<PendingTurn>,
}

/// Which color's move triggered the pending advance.
#[derive(Clone, Copy, Debug, Reflect)]
pub struct PendingTurn {
    pub mover: PieceColor,
}

impl PendingTurnAdvance {
    /// Queues a turn advance for `mover`. Returns `false` (no-op) if one is
    /// already pending — only the first request per turn wins.
    pub fn request(&mut self, mover: PieceColor) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.pending = Some(PendingTurn { mover });
        true
    }

    /// Consumes the pending advance, if any.
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
