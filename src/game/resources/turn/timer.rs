use bevy::prelude::*;

#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct GameTimer {
    pub white_time_left: f32,
    pub black_time_left: f32,
    pub increment: f32,
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
