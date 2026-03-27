//! Game timer resource with Fischer increment support
//!
//! Manages time control for chess games using the Fischer (incremental) time system.
//! Each player starts with a base time and receives an increment after each move.
//!
//! # Fischer Time Control
//!
//! Fischer time control adds a fixed increment to a player's remaining time after
//! they complete their move. This prevents time scrambles and rewards fast play.
//!
//! Example: 10+5 means 10 minutes base time with 5 second increment per move.
//!
//! # Time Management
//!
//! - Timer only runs during `GamePhase::Playing`
//! - Decrements the current player's time each frame
//! - Applies increment after move completion
//! - Sets `GameOverState` when time expires
//!
//! # Reference
//!
//! Fischer increment time control is standard in online chess (Chess.com, Lichess).
//! See: https://en.wikipedia.org/wiki/Time_control#Increment_and_delay_methods

use bevy::prelude::*;

/// Resource for game timer with Fischer increment support
///
/// # Fields
///
/// - `white_time_left`: White's remaining time in seconds
/// - `black_time_left`: Black's remaining time in seconds
/// - `increment`: Time added after each move in seconds (0.0 = no increment)
/// - `is_running`: Whether timer is actively counting down
///
/// # Default Configuration
///
/// Defaults to 10+0 (10 minutes, no increment) with timer paused.
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct GameTimer {
    /// White player's remaining time in seconds
    pub white_time_left: f32,
    /// Black player's remaining time in seconds
    pub black_time_left: f32,
    /// Fischer increment added after each move (in seconds)
    pub increment: f32,
    /// Whether the timer is currently running
    pub is_running: bool,
}

impl Default for GameTimer {
    fn default() -> Self {
        Self {
            white_time_left: 600.0, // 10 minutes
            black_time_left: 600.0,
            increment: 0.0,
            is_running: false,
        }
    }
}

impl GameTimer {
    /// Apply Fischer increment to the player who just moved
    ///
    /// Adds the configured increment time to the player's remaining time.
    /// This is called after a player completes their move.
    ///
    /// # Arguments
    ///
    /// * `color` - The color of the player who just moved
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut timer = GameTimer {
    ///     white_time_left: 300.0,
    ///     black_time_left: 300.0,
    ///     increment: 5.0,  // 5 second increment
    ///     is_running: true,
    /// };
    ///
    /// timer.apply_increment(PieceColor::White);
    /// assert_eq!(timer.white_time_left, 305.0); // 300 + 5
    /// ```
    pub fn apply_increment(&mut self, color: crate::rendering::pieces::PieceColor) {
        use crate::rendering::pieces::PieceColor;

        if self.increment > 0.0 {
            match color {
                PieceColor::White => self.white_time_left += self.increment,
                PieceColor::Black => self.black_time_left += self.increment,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::pieces::PieceColor;

    #[test]
    fn test_game_timer_default() {
        //! Verifies default timer configuration (10 minutes, no increment, paused)
        let timer = GameTimer::default();

        assert_eq!(
            timer.white_time_left, 600.0,
            "White should start with 10 minutes (600 seconds)"
        );
        assert_eq!(
            timer.black_time_left, 600.0,
            "Black should start with 10 minutes (600 seconds)"
        );
        assert_eq!(timer.increment, 0.0, "Default should have no increment");
        assert!(!timer.is_running, "Timer should start paused");
    }

    #[test]
    fn test_apply_increment_white() {
        //! Tests that Fischer increment is added to White's time
        let mut timer = GameTimer {
            white_time_left: 300.0,
            black_time_left: 300.0,
            increment: 5.0,
            is_running: true,
        };

        timer.apply_increment(PieceColor::White);

        assert_eq!(timer.white_time_left, 305.0, "White should gain 5 seconds");
        assert_eq!(
            timer.black_time_left, 300.0,
            "Black's time should not change"
        );
    }

    #[test]
    fn test_apply_increment_black() {
        //! Tests that Fischer increment is added to Black's time
        let mut timer = GameTimer {
            white_time_left: 300.0,
            black_time_left: 300.0,
            increment: 5.0,
            is_running: true,
        };

        timer.apply_increment(PieceColor::Black);

        assert_eq!(
            timer.white_time_left, 300.0,
            "White's time should not change"
        );
        assert_eq!(timer.black_time_left, 305.0, "Black should gain 5 seconds");
    }

    #[test]
    fn test_apply_increment_zero() {
        //! Tests that zero increment doesn't change times
        let mut timer = GameTimer {
            white_time_left: 300.0,
            black_time_left: 300.0,
            increment: 0.0, // No increment
            is_running: true,
        };

        timer.apply_increment(PieceColor::White);
        timer.apply_increment(PieceColor::Black);

        assert_eq!(
            timer.white_time_left, 300.0,
            "Time should not change with 0 increment"
        );
        assert_eq!(
            timer.black_time_left, 300.0,
            "Time should not change with 0 increment"
        );
    }

    #[test]
    fn test_apply_increment_multiple_moves() {
        //! Tests accumulation of increments over multiple moves
        let mut timer = GameTimer {
            white_time_left: 100.0,
            black_time_left: 100.0,
            increment: 3.0,
            is_running: true,
        };

        // Simulate 3 complete moves (6 half-moves)
        timer.apply_increment(PieceColor::White);
        timer.apply_increment(PieceColor::Black);
        timer.apply_increment(PieceColor::White);
        timer.apply_increment(PieceColor::Black);
        timer.apply_increment(PieceColor::White);
        timer.apply_increment(PieceColor::Black);

        assert_eq!(
            timer.white_time_left, 109.0,
            "White should have 3 increments (100 + 9)"
        );
        assert_eq!(
            timer.black_time_left, 109.0,
            "Black should have 3 increments (100 + 9)"
        );
    }

    #[test]
    fn test_fischer_prevents_timeout() {
        //! Tests that increment can prevent flagging even with low time
        let mut timer = GameTimer {
            white_time_left: 1.0, // Only 1 second left
            black_time_left: 300.0,
            increment: 5.0, // But 5 second increment
            is_running: true,
        };

        timer.apply_increment(PieceColor::White);

        assert_eq!(
            timer.white_time_left, 6.0,
            "Increment should save player from timeout"
        );
    }
}
