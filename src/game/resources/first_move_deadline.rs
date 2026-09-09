use bevy::prelude::*;

pub const FIRST_MOVE_GRACE_SECONDS: f32 = 30.0;

#[derive(Resource, Debug, Clone, Default)]
pub struct FirstMoveDeadline {
    pub remaining: f32,
    pub active: bool,
}

impl FirstMoveDeadline {
    pub fn start(&mut self) {
        self.remaining = FIRST_MOVE_GRACE_SECONDS;
        self.active = true;
    }

    pub fn cancel(&mut self) {
        self.active = false;
    }
}
