//! Lifecycle timestamp and timeout helpers.

use crate::state::Game;

pub fn inactivity_window_seconds(game: &Game) -> i64 {
    if game.base_time_seconds > 0 {
        (game.base_time_seconds as i64).saturating_mul(3)
    } else {
        86_400
    }
}

pub fn mark_activity(game: &mut Game, now: i64) {
    game.last_move_timestamp = now;
    game.updated_at = now;
}

pub fn mark_terminal(game: &mut Game, now: i64) {
    game.updated_at = now;
}
