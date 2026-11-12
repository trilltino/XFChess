use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum TurnPhase {
    #[default]
    WaitingForInput,

    PieceSelected,

    ExecutingMove,

    AIThinking,

    CheckingGameState,

    GameOver,
}

#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct TurnStateContext {
    pub current_player: PieceColor,

    pub phase: TurnPhase,

    pub move_number: u32,
}

impl Default for TurnStateContext {
    fn default() -> Self {
        Self {
            current_player: PieceColor::White,
            phase: TurnPhase::WaitingForInput,
            move_number: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_state_context_default() {
        let ctx = TurnStateContext::default();
        assert_eq!(ctx.current_player, PieceColor::White);
        assert_eq!(ctx.phase, TurnPhase::WaitingForInput);
        assert_eq!(ctx.move_number, 1);
    }
}
