//! Turn state management for fine-grained game flow control
//!
//! Tracks the current phase within a player's turn to enable precise
//! system scheduling and prevent race conditions.

use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

/// Fine-grained turn flow state
///
/// Tracks where we are within the current turn, allowing systems to run
/// only when appropriate. This prevents issues like:
/// - AI computing moves during human input
/// - Move validation running before piece selection
/// - Visual updates triggering before move execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum TurnPhase {
    /// Waiting for player to select a piece or make a move
    ///
    /// Active during: Human player's turn
    /// Valid transitions: → PieceSelected, → AIThinking
    #[default]
    WaitingForInput,

    /// Player has selected a piece, showing possible moves
    ///
    /// Active during: After clicking a piece
    /// Valid transitions: → ExecutingMove, → WaitingForInput (cancel)
    PieceSelected,

    /// Move is being executed (update board state, animations)
    ///
    /// Active during: Move execution and animation
    /// Valid transitions: → CheckingGameState
    ExecutingMove,

    /// AI is computing its next move
    ///
    /// Active during: AI opponent's turn
    /// Valid transitions: → ExecutingMove (when AI returns move)
    AIThinking,

    /// Checking for check, checkmate, stalemate
    ///
    /// Active during: After each move completes
    /// Valid transitions: → GameOver, → WaitingForInput
    CheckingGameState,

    /// Game has ended (checkmate, stalemate, time out)
    ///
    /// Terminal state
    GameOver,
}

/// Resource that combines current turn color with turn phase
///
/// This provides context about both WHOSE turn it is and WHAT PHASE
/// of the turn we're in, enabling precise system scheduling.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct TurnStateContext {
    /// Whose turn is it?
    pub current_player: PieceColor,

    /// What phase of the turn are we in?
    pub phase: TurnPhase,

    /// Move number (increments after both players move)
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
