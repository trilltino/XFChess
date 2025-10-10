//! Game timer resource

use bevy::prelude::*;

/// Resource for game timer (optional)
#[derive(Resource, Debug)]
pub struct GameTimer {
    pub white_time_left: f32,  // in seconds
    pub black_time_left: f32,
    #[allow(dead_code)] // TODO: Will be used for Fischer time control
    pub increment: f32,         // Fischer increment
    pub is_running: bool,
}

impl Default for GameTimer {
    fn default() -> Self {
        Self {
            white_time_left: 600.0,  // 10 minutes
            black_time_left: 600.0,
            increment: 0.0,
            is_running: false,
        }
    }
}
