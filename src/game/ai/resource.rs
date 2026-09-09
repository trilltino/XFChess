use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct ChessAIResource {
    pub mode: GameMode,

    pub difficulty: AIDifficulty,

    pub engine: AIEngine,
}

impl Default for ChessAIResource {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum AIEngine {
    Stockfish,
    #[default]
    XFChessEngine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum GameMode {
    Multiplayer,

    MultiplayerCompetitive,

    VsAI { ai_color: PieceColor },
}

impl GameMode {
    pub fn ai_color(self) -> PieceColor {
        match self {
            GameMode::VsAI { ai_color } => ai_color,
            GameMode::Multiplayer | GameMode::MultiplayerCompetitive => PieceColor::Black,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum AIDifficulty {
    Level1,
    Level2,
    Level3,
    #[default]
    Level4,
    Level5,
    Level6,
    Level7,
    Level8,
}

impl AIDifficulty {
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

    pub fn seconds_per_move(self) -> f32 {
        self.stockfish_movetime_ms().unwrap_or(0) as f32 / 1000.0
    }

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
        assert_eq!(AIDifficulty::Level1.seconds_per_move(), 0.05);
        assert_eq!(AIDifficulty::Level4.seconds_per_move(), 0.6);
        assert_eq!(AIDifficulty::Level8.seconds_per_move(), 3.0);
    }

    #[test]
    fn test_ai_difficulty_equality() {
        assert_eq!(AIDifficulty::Level1, AIDifficulty::Level1);
        assert_eq!(AIDifficulty::Level4, AIDifficulty::Level4);
        assert_ne!(AIDifficulty::Level1, AIDifficulty::Level8);
    }

    #[test]
    fn test_ai_difficulty_clone() {
        let original = AIDifficulty::Level8;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_game_mode_clone() {
        let original = GameMode::VsAI {
            ai_color: PieceColor::White,
        };
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_setting_ai_vs_human_mode() {
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
