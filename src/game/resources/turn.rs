//! Turn tracking resource
//!
//! Manages the current player's turn and move counter for chess games.
//! This resource is the single source of truth for whose turn it is.
//!
//! # Architecture
//!
//! - Uses Bevy's `Resource` pattern for global game state
//! - Implements `Reflect` for inspector integration
//! - Move number increments only when White completes their turn
//!
//! # Turn Flow
//!
//! ```text
//! Move 1: White plays → switch() → Black plays → switch() → Move 2: White plays
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! fn execute_move(mut current_turn: ResMut<CurrentTurn>) {
//!     // ... execute the move ...
//!     current_turn.switch(); // Switch to other player
//! }
//! ```
//!
//! # Reference
//!
//! Standard chess turn tracking following FIDE rules where White moves first
//! and move numbers increment after Black's move completes.

use bevy::prelude::*;
use crate::rendering::pieces::PieceColor;

/// Tracks whose turn it currently is and the current move number
///
/// # Fields
///
/// - `color`: The player who should move next (White or Black)
/// - `move_number`: Current move number in chess notation (starts at 1)
///
/// # Move Counting
///
/// Move numbers follow standard chess notation:
/// - Move 1: White's first move and Black's response
/// - Move 2: White's second move and Black's response
/// - Increments only after White completes their turn
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
    /// Switch to the other player's turn
    ///
    /// Increments the move number only when switching from White to Black,
    /// following standard chess notation where a "move" consists of both
    /// White's and Black's turns.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut turn = CurrentTurn::default();
    /// assert_eq!(turn.color, PieceColor::White);
    /// assert_eq!(turn.move_number, 1);
    ///
    /// turn.switch(); // Now Black's turn, still move 1
    /// assert_eq!(turn.color, PieceColor::Black);
    /// assert_eq!(turn.move_number, 1);
    ///
    /// turn.switch(); // Now White's turn, move 2
    /// assert_eq!(turn.color, PieceColor::White);
    /// assert_eq!(turn.move_number, 2);
    /// ```
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

/// Resource to track the current game phase
///
/// Wraps the `GamePhase` component to provide global access to the current
/// game state (Playing, Check, Checkmate, Stalemate).
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
        //! Verifies that games start with White to move on move 1
        let turn = CurrentTurn::default();
        assert_eq!(turn.color, PieceColor::White);
        assert_eq!(turn.move_number, 1);
    }

    #[test]
    fn test_turn_switch_white_to_black() {
        //! Tests switching from White to Black stays on same move number
        let mut turn = CurrentTurn::default();
        turn.switch();

        assert_eq!(turn.color, PieceColor::Black);
        assert_eq!(turn.move_number, 1, "Move number should not increment when White switches to Black");
    }

    #[test]
    fn test_turn_switch_black_to_white() {
        //! Tests switching from Black to White increments the move number
        let mut turn = CurrentTurn {
            color: PieceColor::Black,
            move_number: 1,
        };
        turn.switch();

        assert_eq!(turn.color, PieceColor::White);
        assert_eq!(turn.move_number, 2, "Move number should increment when Black completes their turn");
    }

    #[test]
    fn test_multiple_turn_switches() {
        //! Verifies correct turn tracking over multiple moves
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
        //! Verifies CurrentTurn can be cloned correctly
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
        //! Verifies game phase defaults to Playing
        let phase = CurrentGamePhase::default();
        assert_eq!(phase.0, crate::game::components::GamePhase::Playing);
    }
}
