//! First-move deadline — online games only. If White doesn't play the
//! game's first move within the grace period, the game is aborted (no
//! winner, no rating impact), matching lichess's own "aborted" outcome.

use bevy::prelude::*;

/// Seconds White has to play the first move before the game is aborted.
pub const FIRST_MOVE_GRACE_SECONDS: f32 = 30.0;

/// Tracks the first-move countdown for the active online game.
#[derive(Resource, Debug, Clone, Default)]
pub struct FirstMoveDeadline {
    /// Seconds remaining. Only meaningful while `active`.
    pub remaining: f32,
    /// Whether the countdown is currently running.
    pub active: bool,
}

impl FirstMoveDeadline {
    /// Start (or restart) the grace period.
    pub fn start(&mut self) {
        self.remaining = FIRST_MOVE_GRACE_SECONDS;
        self.active = true;
    }

    /// Stop the countdown without ending the game — called once the first
    /// move has been played.
    pub fn cancel(&mut self) {
        self.active = false;
    }
}
