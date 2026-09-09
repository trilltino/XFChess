use crate::game::time_control::TimeControl;
use bevy::prelude::*;

#[derive(Resource, Debug, Clone)]
pub struct ActiveTimeControl {
    pub control: TimeControl,
    pub ai_game: bool,
}

impl Default for ActiveTimeControl {
    fn default() -> Self {
        Self {
            control: TimeControl::Blitz,
            ai_game: false,
        }
    }
}
