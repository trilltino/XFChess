use crate::state::Game;

pub const TIMED_GAME_INACTIVITY_WINDOW_SECONDS: i64 = 90;
pub const ZERO_MOVE_REFUND_WINDOW_SECONDS: i64 = 90;
const UNTIMED_GAME_INACTIVITY_WINDOW_SECONDS: i64 = 86_400;

pub fn inactivity_window_seconds(game: &Game) -> i64 {
    if game.base_time_seconds > 0 {
        TIMED_GAME_INACTIVITY_WINDOW_SECONDS
    } else {
        UNTIMED_GAME_INACTIVITY_WINDOW_SECONDS
    }
}

pub fn mark_activity(game: &mut Game, now: i64) {
    game.last_move_timestamp = now;
    game.updated_at = now;
}

pub fn mark_terminal(game: &mut Game, now: i64) {
    game.updated_at = now;
}
