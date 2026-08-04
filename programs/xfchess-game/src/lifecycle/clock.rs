//! Lifecycle timestamp and timeout helpers.

use crate::state::Game;

/// How long a game may go without activity before it's eligible for
/// expiry/cancellation: 3× the base time control, or 24h for untimed games.
pub fn inactivity_window_seconds(game: &Game) -> i64 {
    if game.base_time_seconds > 0 {
        (game.base_time_seconds as i64).saturating_mul(3)
    } else {
        86_400
    }
}

/// Records that a move just happened: bumps both the move clock and the
/// general `updated_at` timestamp.
pub fn mark_activity(game: &mut Game, now: i64) {
    game.last_move_timestamp = now;
    game.updated_at = now;
}

/// Bumps `updated_at` when a game reaches a terminal state (no move clock
/// change, since the game is no longer being played).
pub fn mark_terminal(game: &mut Game, now: i64) {
    game.updated_at = now;
}
