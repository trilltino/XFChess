use crate::state::Game;

/// Shortest inactivity window for a timed game; also covers bullet controls
/// whose whole clock is shorter than this.
pub const TIMED_GAME_INACTIVITY_WINDOW_SECONDS: i64 = 90;
pub const ZERO_MOVE_REFUND_WINDOW_SECONDS: i64 = 90;
const UNTIMED_GAME_INACTIVITY_WINDOW_SECONDS: i64 = 86_400;

/// How long the side to move may go without moving before anyone may end the
/// game on time (`claim_timeout` and the ER crank).
///
/// Per-side clocks are not stored on chain, so a timed game's window is the
/// largest clock the side to move can possibly have: the base time plus one
/// increment for each move it has made. A player who still has time on the
/// displayed clock is therefore never forfeited; one who has stopped moving
/// forfeits once even a full clock would have run out.
pub fn inactivity_window_seconds(game: &Game) -> i64 {
    if game.base_time_seconds == 0 {
        return UNTIMED_GAME_INACTIVITY_WINDOW_SECONDS;
    }
    // White moves on odd turns, so each side has made move_count / 2 moves
    // whenever it is that side's turn.
    let moves_by_side_to_move = u64::from(game.move_count / 2);
    let max_clock = game
        .base_time_seconds
        .saturating_add(u64::from(game.increment_seconds).saturating_mul(moves_by_side_to_move));
    i64::try_from(max_clock)
        .unwrap_or(i64::MAX)
        .max(TIMED_GAME_INACTIVITY_WINDOW_SECONDS)
}

pub fn mark_activity(game: &mut Game, now: i64) {
    game.last_move_timestamp = now;
    game.updated_at = now;
}

pub fn mark_terminal(game: &mut Game, now: i64) {
    game.updated_at = now;
}

pub fn timeout_window_seconds(game: &Game) -> i64 {
    if game.move_count == 0 {
        ZERO_MOVE_REFUND_WINDOW_SECONDS
    } else {
        inactivity_window_seconds(game)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{GameResult, GameStatus, GameType, MatchType};
    use anchor_lang::prelude::Pubkey;

    fn game(base_time_seconds: u64, increment_seconds: u16, move_count: u16) -> Game {
        Game {
            game_id: 1,
            white: Pubkey::new_unique(),
            black: Pubkey::new_unique(),
            status: GameStatus::Active,
            last_move_timestamp: 0,
            fees_advanced: 0,
            fee_payer: Pubkey::new_unique(),
            result: GameResult::None,
            board_state: [0; 68],
            move_count,
            halfmove_clock: 0,
            turn: move_count.saturating_add(1),
            created_at: 0,
            updated_at: 0,
            wager_amount: 0,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Free,
            country_fee: 0,
            base_time_seconds,
            increment_seconds,
            bump: 0,
            is_delegated: false,
            tournament_id: None,
            nonce: 0,
            draw_offered_by: None,
        }
    }

    #[test]
    fn timed_window_covers_the_whole_base_clock() {
        assert_eq!(inactivity_window_seconds(&game(300, 0, 10)), 300);
        assert_eq!(inactivity_window_seconds(&game(600, 0, 1)), 600);
    }

    #[test]
    fn timed_window_adds_every_increment_the_mover_has_earned() {
        // After 41 plies black is to move and has made 20 moves.
        assert_eq!(inactivity_window_seconds(&game(600, 5, 41)), 600 + 5 * 20);
        // White to move after 40 plies has also made 20 moves.
        assert_eq!(inactivity_window_seconds(&game(600, 5, 40)), 600 + 5 * 20);
    }

    #[test]
    fn short_controls_keep_the_minimum_window() {
        assert_eq!(
            inactivity_window_seconds(&game(60, 0, 4)),
            TIMED_GAME_INACTIVITY_WINDOW_SECONDS
        );
    }

    #[test]
    fn untimed_and_zero_move_windows_are_unchanged() {
        assert_eq!(inactivity_window_seconds(&game(0, 0, 10)), 86_400);
        assert_eq!(
            timeout_window_seconds(&game(600, 0, 0)),
            ZERO_MOVE_REFUND_WINDOW_SECONDS
        );
    }

    #[test]
    fn extreme_time_controls_do_not_overflow() {
        assert_eq!(
            inactivity_window_seconds(&game(u64::MAX, u16::MAX, u16::MAX)),
            i64::MAX
        );
    }
}
