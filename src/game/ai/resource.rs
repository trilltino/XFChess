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
//! | Difficulty | Time/Move | Typical Depth | Strength          |
//! |------------|-----------|---------------|-------------------|
//! | Level 1    | 0.05s     | 1 ply         | Beginner (400)    |
//! | Level 4    | 0.6s      | 6 ply         | Club (1300)       |
//! | Level 8    | 3.0s      | 24 ply        | Master (2500+)    |
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
//!         difficulty: AIDifficulty::Level4,
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
/// - `difficulty`: AI strength level (Level 1-8)
///
/// # Examples
///
/// ## Setting up human vs AI game
///
/// ```rust,ignore
/// commands.insert_resource(ChessAIResource {
///     mode: GameMode::VsAI { ai_color: PieceColor::Black },
///     difficulty: AIDifficulty::Level4,
/// });
/// ```
///
/// ## Changing difficulty mid-game (for testing)
///
/// ```rust,ignore
///     ai_config.difficulty = AIDifficulty::Level8;
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

    /// AI engine selection
    pub engine: AIEngine,
}

impl Default for ChessAIResource {
    /// Creates a default AI configuration (AI plays Black, Level 4 difficulty)
    ///
    /// Default mode has AI playing Black (standard setup).
    fn default() -> Self {
        Self {
            mode: GameMode::VsAI {
                ai_color: PieceColor::Black,
            },
            difficulty: AIDifficulty::Level4,
            engine: AIEngine::XFChessEngine,
        }
    }
}

/// AI engine selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum AIEngine {
    /// External Stockfish process (High strength, requires stockfish.exe)
    Stockfish,
    /// Internal XFChessEngine (Lightweight, native Rust, no external process)
    #[default]
    XFChessEngine,
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
    /// Multiplayer mode (Local or Online)
    ///
    /// No AI involvement. Both sides controlled by human input (local or network events).
    Multiplayer,

    /// Multiplayer Competitive mode (Ranked)
    MultiplayerCompetitive,

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
            GameMode::Multiplayer | GameMode::MultiplayerCompetitive => PieceColor::Black,
        }
    }
}

/// AI difficulty levels corresponding to search time and depth
///
/// Difficulty determines how long the AI thinks per move, which directly
/// affects search depth thanks to iterative deepening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum AIDifficulty {
    /// Level 1 - ~400 ELO (Complete Beginner)
    Level1,
    /// Level 2 - ~700 ELO (Casual Player)
    Level2,
    /// Level 3 - ~1000 ELO (Amateur)
    Level3,
    /// Level 4 - ~1300 ELO (Club Player)
    #[default]
    Level4,
    /// Level 5 - ~1600 ELO (Intermediate)
    Level5,
    /// Level 6 - ~1900 ELO (Advanced)
    Level6,
    /// Level 7 - ~2200 ELO (Expert)
    Level7,
    /// Level 8 - ~2500+ ELO (Master)
    Level8,
}

impl AIDifficulty {
    /// Convert an integer 1-8 to AIDifficulty
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Level1,
            2 => Self::Level2,
            3 => Self::Level3,
            4 => Self::Level4,
            5 => Self::Level5,
            6 => Self::Level6,
            7 => Self::Level7,
            8 => Self::Level8,
            _ => Self::Level4,
        }
    }

    /// Convert AIDifficulty to an integer 1-8
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Level1 => 1,
            Self::Level2 => 2,
            Self::Level3 => 3,
            Self::Level4 => 4,
            Self::Level5 => 5,
            Self::Level6 => 6,
            Self::Level7 => 7,
            Self::Level8 => 8,
        }
    }

    /// Stockfish search depth for this difficulty.
    pub fn stockfish_depth(self) -> Option<u8> {
        match self {
            Self::Level1 => Some(1),
            Self::Level2 => Some(2),
            Self::Level3 => Some(4),
            Self::Level4 => Some(6),
            Self::Level5 => Some(10),
            Self::Level6 => Some(14),
            Self::Level7 => Some(18),
            Self::Level8 => Some(24),
        }
    }

    /// Maximum search time in milliseconds.
    pub fn stockfish_movetime_ms(self) -> Option<u64> {
        match self {
            Self::Level1 => Some(50),
            Self::Level2 => Some(150),
            Self::Level3 => Some(300),
            Self::Level4 => Some(600),
            Self::Level5 => Some(1000),
            Self::Level6 => Some(1500),
            Self::Level7 => Some(2000),
            Self::Level8 => Some(3000),
        }
    }

    /// Time per move in seconds.
    pub fn seconds_per_move(self) -> f32 {
        self.stockfish_movetime_ms().unwrap_or(0) as f32 / 1000.0
    }

    /// Friendly description for UI with ELO equivalent
    pub fn description(self) -> &'static str {
        match self {
            Self::Level1 => "Beginner (400 ELO)",
            Self::Level2 => "Casual (700 ELO)",
            Self::Level3 => "Amateur (1000 ELO)",
            Self::Level4 => "Club (1300 ELO)",
            Self::Level5 => "Intermediate (1600 ELO)",
            Self::Level6 => "Advanced (1900 ELO)",
            Self::Level7 => "Expert (2200 ELO)",
            Self::Level8 => "Master (2500+ ELO)",
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
        assert_eq!(ai_config.difficulty, AIDifficulty::Level4);
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
        assert_eq!(AIDifficulty::Level1.seconds_per_move(), 0.05);
        assert_eq!(AIDifficulty::Level4.seconds_per_move(), 0.6);
        assert_eq!(AIDifficulty::Level8.seconds_per_move(), 3.0);
    }

    #[test]
    fn test_ai_difficulty_equality() {
        //! Tests AI difficulty comparisons
        assert_eq!(AIDifficulty::Level1, AIDifficulty::Level1);
        assert_eq!(AIDifficulty::Level4, AIDifficulty::Level4);
        assert_ne!(AIDifficulty::Level1, AIDifficulty::Level8);
    }

    #[test]
    fn test_ai_difficulty_clone() {
        //! Verifies AI difficulty can be cloned
        let original = AIDifficulty::Level8;
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
        ai_config.difficulty = AIDifficulty::Level8;

        if let GameMode::VsAI { ai_color } = ai_config.mode {
            assert_eq!(ai_color, PieceColor::Black);
        } else {
            panic!("Expected VsAI mode");
        }

        assert_eq!(ai_config.difficulty, AIDifficulty::Level8);
    }

    #[test]
    fn test_time_increases_with_difficulty() {
        //! Verifies harder difficulties get more thinking time
        let low_time = AIDifficulty::Level1.seconds_per_move();
        let med_time = AIDifficulty::Level4.seconds_per_move();
        let high_time = AIDifficulty::Level8.seconds_per_move();

        assert!(low_time < med_time);
        assert!(med_time < high_time);
    }
}
