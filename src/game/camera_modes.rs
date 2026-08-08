//! Camera lock state: Locked (fixed at the standard position, all controls
//! disabled) or Free (WASDQE pan/rotate + scroll zoom enabled). Toggled via
//! the padlock button in the top HUD bar; 'R' always snaps back to the
//! standard position regardless of lock state.

use bevy::prelude::*;

/// Whether the board camera accepts WASDQE/scroll/mouse-drag input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Resource)]
#[reflect(Resource)]
pub struct CameraLockState {
    /// `true` = camera fixed at the standard position, controls disabled.
    pub locked: bool,
}

impl Default for CameraLockState {
    fn default() -> Self {
        Self { locked: true }
    }
}

impl CameraLockState {
    pub fn toggle(&mut self) {
        self.locked = !self.locked;
    }
}
