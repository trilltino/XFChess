//! Game over state tracking and result management
//!
//! Tracks end-game conditions (checkmate, stalemate, timeout, resignation) and
//! provides methods for determining game outcome and displaying results.
//!
//! # Game Over Conditions
//!
//! ## Checkmate
//! - **WhiteWon**: Black is in check with no legal moves
//! - **BlackWon**: White is in check with no legal moves
//!
//! ## Draw Conditions
//! - **Stalemate**: Current player has no legal moves but is NOT in check
//! - **InsufficientMaterial**: Neither player can deliver checkmate (e.g., K vs K)
//!
//! ## Time Control
//! - **WhiteWonByTime**: Black's time expired
//! - **BlackWonByTime**: White's time expired
//!
//! # Integration
//!
//! This resource is checked by:
//! - [`crate::game::systems::game_logic`] - Sets game over state when detected
//! - [`crate::ui::game_ui`] - Displays game result messages
//! - Game timer systems - Sets timeout wins
//!
//! # Reference
//!
//! FIDE Laws of Chess: https://www.fide.com/FIDE/handbook/LawsOfChess.pdf
//! - Article 5: Checkmate, stalemate, draws
//! - Article 6: Time control violations

use bevy::prelude::*;
use crate::rendering::pieces::PieceColor;

/// Resource tracking the game's end state
///
/// Starts as `Playing` and transitions to a terminal state when the game ends.
/// Once set to any non-Playing state, the game should disable move input and
/// display the result to players.
///
/// # State Transitions
///
/// ```text
/// Playing → WhiteWon / BlackWon / Stalemate / InsufficientMaterial / WhiteWonByTime / BlackWonByTime
/// ```
///
/// All non-Playing states are terminal (game cannot continue).
///
/// # Examples
///
/// ## Detecting checkmate
///
/// ```rust,ignore
/// fn check_for_checkmate(
///     mut game_over: ResMut<GameOverState>,
///     current_turn: Res<CurrentTurn>,
///     // ... other parameters for move validation
/// ) {
///     if is_in_check() && no_legal_moves_available() {
///         *game_over = match current_turn.color {
///             PieceColor::White => GameOverState::BlackWon, // Black checkmated White
///             PieceColor::Black => GameOverState::WhiteWon, // White checkmated Black
///         };
///     }
/// }
/// ```
///
/// ## Displaying result
///
/// ```rust,ignore
/// fn display_game_result(game_over: Res<GameOverState>) {
///     if game_over.is_game_over() {
///         println!("{}", game_over.message());
///
///         if let Some(winner) = game_over.winner() {
///             println!("{:?} won the game!", winner);
///         } else {
///             println!("Game ended in a draw");
///         }
///     }
/// }
/// ```
#[derive(Resource, Default, Debug, Reflect, PartialEq, Eq, Clone, Copy)]
#[reflect(Resource)]
pub enum GameOverState {
    /// Game is still in progress
    ///
    /// Players can make moves, timer is running (if enabled), and game logic
    /// continues to validate moves and check for end conditions.
    #[default]
    Playing,

    /// White won by checkmate
    ///
    /// Black's king is in check and Black has no legal moves to escape check.
    /// This is the standard victory condition in chess.
    WhiteWon,

    /// Black won by checkmate
    ///
    /// White's king is in check and White has no legal moves to escape check.
    BlackWon,

    /// Draw by stalemate
    ///
    /// Current player has no legal moves but is NOT in check. This is an
    /// automatic draw regardless of material advantage.
    ///
    /// Common in endgames when a strong player accidentally prevents the
    /// opponent from moving (e.g., lone king trapped but not in check).
    Stalemate,

    /// Draw by insufficient material
    ///
    /// Neither player has enough pieces to deliver checkmate. Examples:
    /// - King vs King
    /// - King + Bishop vs King
    /// - King + Knight vs King
    ///
    /// Automatically declared as a draw when detected.
    InsufficientMaterial,

    /// White won on time
    ///
    /// Black's time expired before completing their move. Only possible in
    /// timed games (not applicable to unlimited time controls).
    WhiteWonByTime,

    /// Black won on time
    ///
    /// White's time expired before completing their move. Only possible in
    /// timed games.
    BlackWonByTime,
}

impl GameOverState {
    /// Check if the game has ended
    ///
    /// Returns `true` for any non-Playing state, indicating move input should
    /// be disabled and results should be displayed.
    ///
    /// # Returns
    ///
    /// - `true` - Game has ended (checkmate, stalemate, timeout, etc.)
    /// - `false` - Game is still in progress
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn process_input(game_over: Res<GameOverState>) {
    ///     if game_over.is_game_over() {
    ///         return; // Don't process input when game is over
    ///     }
    ///     // ... handle piece selection and moves
    /// }
    /// ```
    pub fn is_game_over(&self) -> bool {
        !matches!(self, GameOverState::Playing)
    }

    /// Get a human-readable message describing the game result
    ///
    /// Returns a message suitable for displaying to players in the UI.
    ///
    /// # Returns
    ///
    /// String describing the game outcome
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if game_over.is_game_over() {
    ///     println!("{}", game_over.message());
    ///     // Displays: "White wins by checkmate!" or "Draw by stalemate", etc.
    /// }
    /// ```
    pub fn message(&self) -> &str {
        match self {
            GameOverState::Playing => "Game in progress",
            GameOverState::WhiteWon => "White wins by checkmate!",
            GameOverState::BlackWon => "Black wins by checkmate!",
            GameOverState::Stalemate => "Draw by stalemate",
            GameOverState::InsufficientMaterial => "Draw by insufficient material",
            GameOverState::WhiteWonByTime => "White wins on time!",
            GameOverState::BlackWonByTime => "Black wins on time!",
        }
    }

    /// Get the winner of the game, if there is one
    ///
    /// Returns `Some(PieceColor)` if one player won, `None` for draws or
    /// ongoing games.
    ///
    /// # Returns
    ///
    /// - `Some(PieceColor::White)` - White won (checkmate or timeout)
    /// - `Some(PieceColor::Black)` - Black won (checkmate or timeout)
    /// - `None` - Draw or game still playing
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// match game_over.winner() {
    ///     Some(PieceColor::White) => println!("White wins!"),
    ///     Some(PieceColor::Black) => println!("Black wins!"),
    ///     None => println!("It's a draw or game is still playing"),
    /// }
    /// ```
    pub fn winner(&self) -> Option<PieceColor> {
        match self {
            GameOverState::WhiteWon | GameOverState::WhiteWonByTime => Some(PieceColor::White),
            GameOverState::BlackWon | GameOverState::BlackWonByTime => Some(PieceColor::Black),
            _ => None,
        }
    }

    /// Check if the game ended in a draw
    ///
    /// Returns `true` for stalemate and insufficient material.
    ///
    /// # Returns
    ///
    /// `true` if game ended in a draw, `false` otherwise
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if game_over.is_draw() {
    ///     println!("Game ended in a draw: {}", game_over.message());
    /// }
    /// ```
    pub fn is_draw(&self) -> bool {
        matches!(self, GameOverState::Stalemate | GameOverState::InsufficientMaterial)
    }

    /// Check if the game ended by timeout
    ///
    /// Returns `true` if either player ran out of time.
    ///
    /// # Returns
    ///
    /// `true` if game ended on time, `false` otherwise
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if game_over.is_timeout() {
    ///     println!("Game ended on time!");
    /// }
    /// ```
    pub fn is_timeout(&self) -> bool {
        matches!(self, GameOverState::WhiteWonByTime | GameOverState::BlackWonByTime)
    }

    /// Check if the game ended by checkmate
    ///
    /// Returns `true` if either player delivered checkmate.
    ///
    /// # Returns
    ///
    /// `true` if game ended in checkmate, `false` otherwise
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if game_over.is_checkmate() {
    ///     println!("Checkmate! {}", game_over.message());
    /// }
    /// ```
    pub fn is_checkmate(&self) -> bool {
        matches!(self, GameOverState::WhiteWon | GameOverState::BlackWon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_over_state_default() {
        //! Verifies GameOverState defaults to Playing
        let state = GameOverState::default();
        assert_eq!(state, GameOverState::Playing);
        assert!(!state.is_game_over());
    }

    #[test]
    fn test_is_game_over_playing() {
        //! Tests that Playing state is not game over
        let state = GameOverState::Playing;
        assert!(!state.is_game_over());
    }

    #[test]
    fn test_is_game_over_white_won() {
        //! Tests that WhiteWon is game over
        let state = GameOverState::WhiteWon;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_black_won() {
        //! Tests that BlackWon is game over
        let state = GameOverState::BlackWon;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_stalemate() {
        //! Tests that Stalemate is game over
        let state = GameOverState::Stalemate;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_insufficient_material() {
        //! Tests that InsufficientMaterial is game over
        let state = GameOverState::InsufficientMaterial;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_timeout_white() {
        //! Tests that WhiteWonByTime is game over
        let state = GameOverState::WhiteWonByTime;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_timeout_black() {
        //! Tests that BlackWonByTime is game over
        let state = GameOverState::BlackWonByTime;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_message_playing() {
        //! Tests message for Playing state
        let state = GameOverState::Playing;
        assert_eq!(state.message(), "Game in progress");
    }

    #[test]
    fn test_message_white_won() {
        //! Tests message for White checkmate victory
        let state = GameOverState::WhiteWon;
        assert_eq!(state.message(), "White wins by checkmate!");
    }

    #[test]
    fn test_message_black_won() {
        //! Tests message for Black checkmate victory
        let state = GameOverState::BlackWon;
        assert_eq!(state.message(), "Black wins by checkmate!");
    }

    #[test]
    fn test_message_stalemate() {
        //! Tests message for stalemate draw
        let state = GameOverState::Stalemate;
        assert_eq!(state.message(), "Draw by stalemate");
    }

    #[test]
    fn test_message_insufficient_material() {
        //! Tests message for insufficient material draw
        let state = GameOverState::InsufficientMaterial;
        assert_eq!(state.message(), "Draw by insufficient material");
    }

    #[test]
    fn test_message_white_won_by_time() {
        //! Tests message for White timeout victory
        let state = GameOverState::WhiteWonByTime;
        assert_eq!(state.message(), "White wins on time!");
    }

    #[test]
    fn test_message_black_won_by_time() {
        //! Tests message for Black timeout victory
        let state = GameOverState::BlackWonByTime;
        assert_eq!(state.message(), "Black wins on time!");
    }

    #[test]
    fn test_winner_white_checkmate() {
        //! Tests winner detection for White checkmate
        let state = GameOverState::WhiteWon;
        assert_eq!(state.winner(), Some(PieceColor::White));
    }

    #[test]
    fn test_winner_black_checkmate() {
        //! Tests winner detection for Black checkmate
        let state = GameOverState::BlackWon;
        assert_eq!(state.winner(), Some(PieceColor::Black));
    }

    #[test]
    fn test_winner_white_timeout() {
        //! Tests winner detection for White timeout win
        let state = GameOverState::WhiteWonByTime;
        assert_eq!(state.winner(), Some(PieceColor::White));
    }

    #[test]
    fn test_winner_black_timeout() {
        //! Tests winner detection for Black timeout win
        let state = GameOverState::BlackWonByTime;
        assert_eq!(state.winner(), Some(PieceColor::Black));
    }

    #[test]
    fn test_winner_stalemate() {
        //! Tests that stalemate has no winner
        let state = GameOverState::Stalemate;
        assert_eq!(state.winner(), None);
    }

    #[test]
    fn test_winner_insufficient_material() {
        //! Tests that insufficient material has no winner
        let state = GameOverState::InsufficientMaterial;
        assert_eq!(state.winner(), None);
    }

    #[test]
    fn test_winner_playing() {
        //! Tests that ongoing game has no winner
        let state = GameOverState::Playing;
        assert_eq!(state.winner(), None);
    }

    #[test]
    fn test_is_draw_stalemate() {
        //! Tests that stalemate is identified as a draw
        let state = GameOverState::Stalemate;
        assert!(state.is_draw());
    }

    #[test]
    fn test_is_draw_insufficient_material() {
        //! Tests that insufficient material is identified as a draw
        let state = GameOverState::InsufficientMaterial;
        assert!(state.is_draw());
    }

    #[test]
    fn test_is_draw_white_won() {
        //! Tests that checkmate is not a draw
        let state = GameOverState::WhiteWon;
        assert!(!state.is_draw());
    }

    #[test]
    fn test_is_draw_timeout() {
        //! Tests that timeout is not a draw
        let state = GameOverState::WhiteWonByTime;
        assert!(!state.is_draw());
    }

    #[test]
    fn test_is_timeout_white() {
        //! Tests timeout detection for White win
        let state = GameOverState::WhiteWonByTime;
        assert!(state.is_timeout());
    }

    #[test]
    fn test_is_timeout_black() {
        //! Tests timeout detection for Black win
        let state = GameOverState::BlackWonByTime;
        assert!(state.is_timeout());
    }

    #[test]
    fn test_is_timeout_checkmate() {
        //! Tests that checkmate is not a timeout
        let state = GameOverState::WhiteWon;
        assert!(!state.is_timeout());
    }

    #[test]
    fn test_is_checkmate_white_won() {
        //! Tests checkmate detection for White win
        let state = GameOverState::WhiteWon;
        assert!(state.is_checkmate());
    }

    #[test]
    fn test_is_checkmate_black_won() {
        //! Tests checkmate detection for Black win
        let state = GameOverState::BlackWon;
        assert!(state.is_checkmate());
    }

    #[test]
    fn test_is_checkmate_timeout() {
        //! Tests that timeout is not checkmate
        let state = GameOverState::WhiteWonByTime;
        assert!(!state.is_checkmate());
    }

    #[test]
    fn test_is_checkmate_draw() {
        //! Tests that draws are not checkmate
        let state = GameOverState::Stalemate;
        assert!(!state.is_checkmate());
    }

    #[test]
    fn test_game_over_state_equality() {
        //! Tests that GameOverState implements PartialEq correctly
        assert_eq!(GameOverState::Playing, GameOverState::Playing);
        assert_eq!(GameOverState::WhiteWon, GameOverState::WhiteWon);
        assert_ne!(GameOverState::WhiteWon, GameOverState::BlackWon);
        assert_ne!(GameOverState::Stalemate, GameOverState::InsufficientMaterial);
    }

    #[test]
    fn test_game_over_state_clone() {
        //! Tests that GameOverState can be cloned
        let original = GameOverState::Stalemate;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_game_over_state_copy() {
        //! Tests that GameOverState implements Copy
        let original = GameOverState::WhiteWon;
        let copied = original; // Copy, not move
        assert_eq!(original, copied);
        assert_eq!(original, GameOverState::WhiteWon); // Original still accessible
    }

    #[test]
    fn test_all_end_states_are_game_over() {
        //! Comprehensive test ensuring all non-Playing states are game over
        let states = vec![
            GameOverState::WhiteWon,
            GameOverState::BlackWon,
            GameOverState::Stalemate,
            GameOverState::InsufficientMaterial,
            GameOverState::WhiteWonByTime,
            GameOverState::BlackWonByTime,
        ];

        for state in states {
            assert!(state.is_game_over(), "{:?} should be game over", state);
        }
    }
}
