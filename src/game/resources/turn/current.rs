use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct CurrentTurn {
    pub color: PieceColor,
    pub move_number: u32,
}

impl Default for CurrentTurn {
    fn default() -> Self {
        Self {
            color: PieceColor::White,
            move_number: 1,
        }
    }
}

impl CurrentTurn {
    pub fn switch(&mut self) {
        self.color = match self.color {
            PieceColor::White => PieceColor::Black,
            PieceColor::Black => {
                self.move_number += 1;
                PieceColor::White
            }
        };
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct CurrentGamePhase(pub crate::game::components::GamePhase);

impl Default for CurrentGamePhase {
    fn default() -> Self {
        Self(crate::game::components::GamePhase::Playing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_turn_default() {
        let turn = CurrentTurn::default();
        assert_eq!(turn.color, PieceColor::White);
        assert_eq!(turn.move_number, 1);
    }

    #[test]
    fn test_turn_switch_white_to_black() {
        let mut turn = CurrentTurn::default();
        turn.switch();

        assert_eq!(turn.color, PieceColor::Black);
        assert_eq!(
            turn.move_number, 1,
            "Move number should not increment when White switches to Black"
        );
    }

    #[test]
    fn test_turn_switch_black_to_white() {
        let mut turn = CurrentTurn {
            color: PieceColor::Black,
            move_number: 1,
        };
        turn.switch();

        assert_eq!(turn.color, PieceColor::White);
        assert_eq!(
            turn.move_number, 2,
            "Move number should increment when Black completes their turn"
        );
    }

    #[test]
    fn test_multiple_turn_switches() {
        let mut turn = CurrentTurn::default();

        // Move 1: White → Black
        turn.switch();
        assert_eq!(turn.color, PieceColor::Black);
        assert_eq!(turn.move_number, 1);

        // Move 1: Black → Move 2: White
        turn.switch();
        assert_eq!(turn.color, PieceColor::White);
        assert_eq!(turn.move_number, 2);

        // Move 2: White → Black
        turn.switch();
        assert_eq!(turn.color, PieceColor::Black);
        assert_eq!(turn.move_number, 2);

        // Move 2: Black → Move 3: White
        turn.switch();
        assert_eq!(turn.color, PieceColor::White);
        assert_eq!(turn.move_number, 3);
    }

    #[test]
    fn test_current_turn_clone() {
        let turn1 = CurrentTurn {
            color: PieceColor::Black,
            move_number: 42,
        };
        let turn2 = turn1.clone();

        assert_eq!(turn1, turn2);
        assert_eq!(turn2.color, PieceColor::Black);
        assert_eq!(turn2.move_number, 42);
    }

    #[test]
    fn test_current_game_phase_default() {
        let phase = CurrentGamePhase::default();
        assert_eq!(phase.0, crate::game::components::GamePhase::Playing);
    }
}
