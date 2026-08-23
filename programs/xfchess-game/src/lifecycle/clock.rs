//! Lifecycle timestamp and timeout helpers.

use crate::state::Game;

/// How long a game may go without activity before it's eligible for
/// expiry/cancellation: a fixed short window for timed games (abandonment
/// should resolve fast — the game's own chess clock already governs
/// per-move time pressure, this window only exists to catch "opponent
/// vanished and never sent another move at all"), or 24h for untimed games
/// (no clock pressure backs a short window there, so a short fixed value
/// would punish legitimate long think time).
///
/// Single source of truth for both the manual `ClaimTimeout` instruction
/// (`game_ix/timeout.rs` via `finish_by_timeout`) and the autonomous ER
/// crank (`crank_ix/crank_time_check.rs` via `finish_by_timeout_if_expired`)
/// — see docs/PRE_MAINNET_E2E_PLAN.md §1.3 on keeping those two in sync.
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
