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
//! AI difficulty combines a search budget with Stockfish strength limiting on
//! the lower levels. Exact strength varies by engine version and hardware.
//!
//! Lower levels use `UCI_LimitStrength`/`UCI_Elo`; the strongest levels use
//! unrestricted Stockfish. Search time is also capped by the selected game
//! time control.
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

    /// Stockfish's native playing-strength target for the lower levels.
    /// `None` leaves the strongest levels unrestricted instead of pretending
    /// that a fixed Elo value is universal across hardware and versions.
    pub fn stockfish_elo(self) -> Option<u16> {
        match self {
            Self::Level1 => Some(1320),
            Self::Level2 => Some(1450),
            Self::Level3 => Some(1600),
            Self::Level4 => Some(1750),
            Self::Level5 => Some(1900),
            Self::Level6 => Some(2100),
            Self::Level7 | Self::Level8 => None,
        }
    }

    /// Time per move in seconds.
    pub fn seconds_per_move(self) -> f32 {
        self.stockfish_movetime_ms().unwrap_or(0) as f32 / 1000.0
    }

    /// Minimum/maximum artificial "thinking" delay (milliseconds) enforced
    /// between the AI's turn starting and its move landing on the board.
    ///
    /// This is a floor, not an addition on top of search time: at low
    /// difficulties the search itself finishes in well under 100ms (Level1
    /// is a 1-ply search), which made the move snap onto the board
    /// instantly and read as robotic. At higher difficulties real search
    /// time (see [`Self::stockfish_movetime_ms`]) already exceeds this
    /// floor, so it adds little or nothing there. The random range (rather
    /// than a fixed pause) avoids every move taking a suspiciously
    /// identical amount of time.
    pub fn thinking_delay_range_ms(self) -> (u64, u64) {
        match self {
            Self::Level1 => (450, 900),
            Self::Level2 => (400, 800),
            Self::Level3 => (350, 700),
            Self::Level4 => (300, 600),
            Self::Level5 => (250, 500),
            Self::Level6 => (200, 400),
            Self::Level7 => (150, 300),
            Self::Level8 => (100, 200),
        }
    }

    /// Legacy display label for callers that still show a compact strength.
    /// These are Stockfish targets, not universal player ratings.
    pub fn elo_label(self) -> &'static str {
        match self {
            Self::Level1 => "1320 target",
            Self::Level2 => "1450 target",
            Self::Level3 => "1600 target",
            Self::Level4 => "1750 target",
            Self::Level5 => "1900 target",
            Self::Level6 => "2100 target",
            Self::Level7 | Self::Level8 => "unrestricted",
        }
    }

    /// Friendly description for the player-facing difficulty selector.
    pub fn description(self) -> &'static str {
        match self {
            Self::Level1 => "Beginner",
            Self::Level2 => "Casual",
            Self::Level3 => "Amateur",
            Self::Level4 => "Club",
            Self::Level5 => "Intermediate",
            Self::Level6 => "Advanced",
            Self::Level7 => "Expert",
            Self::Level8 => "Master",
        }
    }

    /// Longer hover-tooltip text describing the practical playing behavior of
    /// this level without presenting an uncalibrated player rating as fact.
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Level1 => {
                "Beginner\nFrequent tactical mistakes and missed threats."
            }
            Self::Level2 => {
                "Casual\nAvoids some simple blunders but still misses many short tactics."
            }
            Self::Level3 => {
                "Amateur\nSpots basic tactics and short-term threats, with noticeable positional mistakes."
            }
            Self::Level4 => {
                "Club\nA balanced opponent with reliable tactics and occasional inaccuracies."
            }
            Self::Level5 => {
                "Intermediate\nReliable tactical and positional play. Mistakes are less frequent and less obvious."
            }
            Self::Level6 => {
                "Advanced\nStrong tactical vision and consistent punishment of obvious errors."
            }
            Self::Level7 => {
                "Expert\nVery difficult to outplay tactically, with strong calculation and consistent pressure."
            }
            Self::Level8 => {
                "Master\nPrecise, punishing play with very few tactical mistakes."
            }
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

    #[test]
    fn stockfish_strength_policy_is_monotonic_and_bounded() {
        let levels = [
            AIDifficulty::Level1,
            AIDifficulty::Level2,
            AIDifficulty::Level3,
            AIDifficulty::Level4,
            AIDifficulty::Level5,
            AIDifficulty::Level6,
            AIDifficulty::Level7,
            AIDifficulty::Level8,
        ];
        let mut previous_elo = 0;
        for level in levels {
            if let Some(elo) = level.stockfish_elo() {
                assert!((1320..=2100).contains(&elo));
                assert!(elo > previous_elo);
                previous_elo = elo;
            }
        }
        assert_eq!(AIDifficulty::Level7.stockfish_elo(), None);
        assert_eq!(AIDifficulty::Level8.stockfish_elo(), None);
    }

    #[test]
    fn every_difficulty_has_a_search_budget() {
        for value in 1..=8 {
            let level = AIDifficulty::from_u8(value);
            assert!(level.stockfish_movetime_ms().unwrap() > 0);
            assert!(level.stockfish_depth().unwrap() > 0);
        }
    }
}
