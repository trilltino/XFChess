use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Resource)]
#[reflect(Resource)]
pub struct CameraLockState {
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
