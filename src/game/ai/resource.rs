//! AI resource definitions for game mode and difficulty settings
//!
//! Configures AI opponent behavior, strength, and game mode selection.
//! These resources control when the AI activates and how strong it plays.
//!
//! # Game Modes
//!
//! - **VsHuman**: Two human players (local hot-seat multiplayer)
//! - **VsAI**: One human player vs AI opponent (specify AI color)
//!
//! # Difficulty Levels
//!
//! AI difficulty is controlled by search time, which directly affects search depth:
//!
//! | Difficulty | Time/Move | Typical Depth | Strength       |
//! |------------|-----------|---------------|----------------|
//! | Easy       | 0.5s      | 3-4 ply       | Beginner (800) |
//! | Medium     | 1.5s      | 5-6 ply       | Club (1400)    |
//! | Hard       | 3.0s      | 7-8 ply       | Strong (1800+) |
//!
//! Depth increases with search time thanks to iterative deepening in the engine.
//!
//! # Integration
//!
//! The UI menu sets these resources based on player selection:
//!
//! ```rust,ignore
//! fn start_vs_ai_game(mut commands: Commands) {
//!     commands.insert_resource(ChessAIResource {
//!         mode: GameMode::VsAI { ai_color: PieceColor::Black },
//!         difficulty: AIDifficulty::Medium,
//!     });
//! }
//! ```
//!
//! The [`crate::game::ai::systems`] module checks these resources to determine
//! when to spawn AI move computation tasks.
//!
//! # Reference
//!
//! Chess engine strength analysis:
//! - `crates/chess_engine/README.md` - Engine architecture and strength
//! - ELO ratings are approximate based on depth-to-strength correlation studies

use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

/// Main resource for chess AI configuration
///
/// Controls game mode (human vs human or human vs AI) and AI difficulty.
/// Updated by the launch menu UI when starting a new game.
///
/// # Fields
///
/// - `mode`: Current game mode (VsHuman or VsAI with color specification)
/// - `difficulty`: AI strength level (Easy/Medium/Hard)
///
/// # Examples
///
/// ## Setting up human vs AI game
///
/// ```rust,ignore
/// commands.insert_resource(ChessAIResource {
///     mode: GameMode::VsAI { ai_color: PieceColor::Black },
///     difficulty: AIDifficulty::Medium,
/// });
/// ```
///
/// ## Changing difficulty mid-game (for testing)
///
/// ```rust,ignore
/// fn change_difficulty(mut ai_config: ResMut<ChessAIResource>) {
///     ai_config.difficulty = AIDifficulty::Hard;
///     info!("AI now using {}", ai_config.difficulty.description());
/// }
/// ```
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct ChessAIResource {
    /// Current game mode (vs human or vs AI)
    ///
    /// Determines whether AI systems should be active. When `VsHuman`,
    /// AI systems early-return without computation. When `VsAI`,
    /// AI spawns move computation tasks when `ai_color` matches current turn.
    pub mode: GameMode,

    /// AI difficulty setting
    ///
    /// Controls search time per move, which determines search depth
    /// and playing strength. Higher difficulties search deeper but
    /// take longer to respond.
    pub difficulty: AIDifficulty,
}

impl Default for ChessAIResource {
    /// Creates a default AI configuration (AI plays Black, medium difficulty)
    ///
    /// Default mode has AI playing Black (standard setup).
    fn default() -> Self {
        Self {
            mode: GameMode::VsAI {
                ai_color: PieceColor::Black,
            },
            difficulty: AIDifficulty::Medium,
        }
    }
}

/// Game mode selection
///
/// Determines which color the AI plays.
///
/// # Variants
///
/// - **VsAI**: One human vs AI opponent
///
/// # Examples
///
/// ```rust,ignore
/// // Human plays White, AI plays Black
/// let mode = GameMode::VsAI { ai_color: PieceColor::Black };
///
/// // AI plays White, Human plays Black
/// let mode = GameMode::VsAI { ai_color: PieceColor::White };
/// ```
///
/// # Usage in Systems
///
/// Systems check the mode to determine if AI should activate:
///
/// ```rust,ignore
/// fn spawn_ai_task_system(ai_config: Res<ChessAIResource>, current_turn: Res<CurrentTurn>) {
///     let ai_color = ai_config.mode.ai_color;
///
///     if current_turn.color != ai_color {
///         return; // Not AI's turn
///     }
///
///     // Spawn AI move computation...
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum GameMode {
    /// Human vs AI opponent
    ///
    /// The specified color is controlled by the AI engine.
    /// The other color is controlled by human input.
    ///
    /// # Field
    ///
    /// - `ai_color`: Which color the AI plays (White or Black)
    VsAI {
        /// The color the AI will play
        ///
        /// When `current_turn.color == ai_color`, AI systems spawn
        /// a move computation task.
        ai_color: PieceColor,
    },
}

impl GameMode {
    /// Get the AI player's color
    ///
    /// Returns the color that the AI is playing.
    pub fn ai_color(self) -> PieceColor {
        match self {
            GameMode::VsAI { ai_color } => ai_color,
        }
    }
}

/// AI difficulty levels corresponding to search time and depth
///
/// Difficulty determines how long the AI thinks per move, which directly
/// affects search depth thanks to iterative deepening. The engine searches
/// progressively deeper until time runs out.
///
/// # Strength vs Performance Trade-off
///
/// - **Easy**: Fast responses, suitable for beginners or low-end hardware
/// - **Medium**: Balanced strength and response time
/// - **Hard**: Strong play, but may cause frame drops on slow hardware
///
/// # Implementation
///
/// The engine uses:
/// - **Iterative Deepening**: Searches depth 1, then 2, then 3, etc. until time runs out
/// - **Alpha-Beta Pruning**: Skips branches that can't improve the best move
/// - **Transposition Tables**: Caches positions to avoid re-searching
///
/// More time → more depth → stronger play
///
/// # Examples
///
/// ```rust,ignore
/// let difficulty = AIDifficulty::Medium;
/// println!("AI will think for {} seconds", difficulty.seconds_per_move());
/// println!("Difficulty: {}", difficulty.description());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum AIDifficulty {
    /// Easy - 0.5 seconds per move, depth ~3-4 ply
    ///
    /// Beginner level (~800 ELO estimate). Makes obvious mistakes,
    /// doesn't see tactical threats beyond 2-3 moves ahead.
    ///
    /// Good for:
    /// - New chess players
    /// - Testing/debugging
    /// - Low-end hardware
    Easy,

    /// Medium - 1.5 seconds per move, depth ~5-6 ply
    ///
    /// Club player level (~1400 ELO estimate). Sees most tactical
    /// combinations, plays solid positional chess.
    ///
    /// Good for:
    /// - Intermediate players
    /// - Casual games
    /// - Default difficulty
    Medium,

    /// Hard - 3.0 seconds per move, depth ~7-8 ply
    ///
    /// Strong player level (~1800+ ELO estimate). Finds complex
    /// tactics, plays excellent endgames, rarely makes mistakes.
    ///
    /// Good for:
    /// - Advanced players
    /// - Serious practice
    /// - High-end hardware
    Hard,
}

impl AIDifficulty {
    /// Get the time allocation for this difficulty level
    ///
    /// Returns the number of seconds the AI will think per move.
    /// The chess engine uses this for its time management and
    /// iterative deepening control.
    ///
    /// # Returns
    ///
    /// Time in seconds as f32:
    /// - Easy: 0.5s
    /// - Medium: 1.5s
    /// - Hard: 3.0s
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let difficulty = AIDifficulty::Hard;
    /// engine.secs_per_move = difficulty.seconds_per_move();
    /// // AI will now think for 3.0 seconds
    /// ```
    pub fn seconds_per_move(self) -> f32 {
        match self {
            AIDifficulty::Easy => 0.5,
            AIDifficulty::Medium => 1.5,
            AIDifficulty::Hard => 3.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chess_ai_resource_default() {
        //! Verifies ChessAIResource defaults to VsAI mode with Black AI
        let ai_config = ChessAIResource::default();
        assert!(matches!(
            ai_config.mode,
            GameMode::VsAI {
                ai_color: PieceColor::Black
            }
        ));
        assert_eq!(ai_config.difficulty, AIDifficulty::Medium);
    }

    #[test]
    fn test_game_mode_equality() {
        //! Tests GameMode equality comparisons
        assert_eq!(
            GameMode::VsAI {
                ai_color: PieceColor::White
            },
            GameMode::VsAI {
                ai_color: PieceColor::White
            }
        );
        assert_ne!(
            GameMode::VsAI {
                ai_color: PieceColor::White
            },
            GameMode::VsAI {
                ai_color: PieceColor::Black
            }
        );
    }

    #[test]
    fn test_ai_difficulty_time_allocation() {
        //! Verifies difficulty time allocations are correct
        assert_eq!(AIDifficulty::Easy.seconds_per_move(), 0.5);
        assert_eq!(AIDifficulty::Medium.seconds_per_move(), 1.5);
        assert_eq!(AIDifficulty::Hard.seconds_per_move(), 3.0);
    }

    #[test]
    fn test_ai_difficulty_equality() {
        //! Tests AI difficulty comparisons
        assert_eq!(AIDifficulty::Easy, AIDifficulty::Easy);
        assert_eq!(AIDifficulty::Medium, AIDifficulty::Medium);
        assert_ne!(AIDifficulty::Easy, AIDifficulty::Hard);
    }

    #[test]
    fn test_ai_difficulty_clone() {
        //! Verifies AI difficulty can be cloned
        let original = AIDifficulty::Hard;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_game_mode_clone() {
        //! Verifies GameMode can be cloned
        let original = GameMode::VsAI {
            ai_color: PieceColor::White,
        };
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_setting_ai_vs_human_mode() {
        //! Tests configuring AI resource for human vs AI game
        let mut ai_config = ChessAIResource::default();

        // Start human vs AI with AI playing black
        ai_config.mode = GameMode::VsAI {
            ai_color: PieceColor::Black,
        };
        ai_config.difficulty = AIDifficulty::Hard;

        if let GameMode::VsAI { ai_color } = ai_config.mode {
            assert_eq!(ai_color, PieceColor::Black);
        } else {
            panic!("Expected VsAI mode");
        }

        assert_eq!(ai_config.difficulty, AIDifficulty::Hard);
    }

    #[test]
    fn test_time_increases_with_difficulty() {
        //! Verifies harder difficulties get more thinking time
        let easy_time = AIDifficulty::Easy.seconds_per_move();
        let medium_time = AIDifficulty::Medium.seconds_per_move();
        let hard_time = AIDifficulty::Hard.seconds_per_move();

        assert!(easy_time < medium_time);
        assert!(medium_time < hard_time);
    }
}
