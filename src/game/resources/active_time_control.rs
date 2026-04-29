//! Active time control resource — stores the time control chosen before
//! the game started so that `game_init` can seed `GameTimer` correctly.

use bevy::prelude::*;
use crate::game::time_control::TimeControl;

/// Resource inserted at game-start with the chosen time control.
///
/// Set this before transitioning to `GameState::InGame` so that
/// `reset_game_resources` can initialize `GameTimer` from it.
#[derive(Resource, Debug, Clone)]
pub struct ActiveTimeControl {
    /// The selected time control preset.
    pub control: TimeControl,
    /// If `true` the active player is human vs AI — only the human clock ticks.
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
