//! Turn state management for fine-grained game flow control
//!
//! Tracks the current phase within a player's turn to enable precise
//! system scheduling and prevent race conditions.

use bevy::prelude::*;
use crate::rendering::pieces::PieceColor;

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

impl TurnPhase {
    /// Check if this state allows human input
    pub fn accepts_input(&self) -> bool {
        matches!(self, TurnPhase::WaitingForInput | TurnPhase::PieceSelected)
    }

    /// Check if AI should be computing
    pub fn is_ai_thinking(&self) -> bool {
        matches!(self, TurnPhase::AIThinking)
    }

    /// Check if moves are being executed
    pub fn is_executing(&self) -> bool {
        matches!(self, TurnPhase::ExecutingMove | TurnPhase::CheckingGameState)
    }
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

impl TurnStateContext {
    /// Transition to the next turn phase
    ///
    /// # Arguments
    /// * `next_phase` - The phase to transition to
    ///
    /// # Panics
    /// In debug builds, panics if the transition is invalid
    pub fn transition_to(&mut self, next_phase: TurnPhase) {
        #[cfg(debug_assertions)]
        {
            // Validate state transition in debug builds
            let valid = match (self.phase, next_phase) {
                (TurnPhase::WaitingForInput, TurnPhase::PieceSelected) => true,
                (TurnPhase::WaitingForInput, TurnPhase::AIThinking) => true,
                (TurnPhase::PieceSelected, TurnPhase::ExecutingMove) => true,
                (TurnPhase::PieceSelected, TurnPhase::WaitingForInput) => true, // Cancel
                (TurnPhase::ExecutingMove, TurnPhase::CheckingGameState) => true,
                (TurnPhase::AIThinking, TurnPhase::ExecutingMove) => true,
                (TurnPhase::CheckingGameState, TurnPhase::WaitingForInput) => true,
                (TurnPhase::CheckingGameState, TurnPhase::GameOver) => true,
                (TurnPhase::GameOver, _) => false, // Terminal state
                _ => false,
            };

            if !valid {
                panic!(
                    "Invalid turn state transition: {:?} -> {:?}",
                    self.phase, next_phase
                );
            }
        }

        self.phase = next_phase;
    }

    /// Switch to the next player's turn
    ///
    /// Increments move number and resets to WaitingForInput phase
    pub fn switch_turn(&mut self) {
        self.current_player = match self.current_player {
            PieceColor::White => PieceColor::Black,
            PieceColor::Black => {
                self.move_number += 1;
                PieceColor::White
            }
        };
        self.phase = TurnPhase::WaitingForInput;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_phase_accepts_input() {
        assert!(TurnPhase::WaitingForInput.accepts_input());
        assert!(TurnPhase::PieceSelected.accepts_input());
        assert!(!TurnPhase::ExecutingMove.accepts_input());
        assert!(!TurnPhase::AIThinking.accepts_input());
    }

    #[test]
    fn test_turn_state_context_default() {
        let ctx = TurnStateContext::default();
        assert_eq!(ctx.current_player, PieceColor::White);
        assert_eq!(ctx.phase, TurnPhase::WaitingForInput);
        assert_eq!(ctx.move_number, 1);
    }

    #[test]
    fn test_turn_state_context_switch() {
        let mut ctx = TurnStateContext::default();

        ctx.switch_turn();
        assert_eq!(ctx.current_player, PieceColor::Black);
        assert_eq!(ctx.move_number, 1);

        ctx.switch_turn();
        assert_eq!(ctx.current_player, PieceColor::White);
        assert_eq!(ctx.move_number, 2);
    }

    #[test]
    fn test_valid_transitions() {
        let mut ctx = TurnStateContext::default();

        // WaitingForInput -> PieceSelected
        ctx.transition_to(TurnPhase::PieceSelected);
        assert_eq!(ctx.phase, TurnPhase::PieceSelected);

        // PieceSelected -> ExecutingMove
        ctx.transition_to(TurnPhase::ExecutingMove);
        assert_eq!(ctx.phase, TurnPhase::ExecutingMove);

        // ExecutingMove -> CheckingGameState
        ctx.transition_to(TurnPhase::CheckingGameState);
        assert_eq!(ctx.phase, TurnPhase::CheckingGameState);

        // CheckingGameState -> WaitingForInput
        ctx.transition_to(TurnPhase::WaitingForInput);
        assert_eq!(ctx.phase, TurnPhase::WaitingForInput);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Invalid turn state transition")]
    fn test_invalid_transition_panics() {
        let mut ctx = TurnStateContext::default();
        // Can't go directly from WaitingForInput to ExecutingMove
        ctx.transition_to(TurnPhase::ExecutingMove);
    }
}
