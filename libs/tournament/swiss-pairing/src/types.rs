use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwissPlayer {
    pub id: String,
    pub rating: u32,
    pub score: f64,
    pub color_history: Vec<Color>,
    pub opponents: Vec<String>,
    pub bye_rounds: Vec<u8>,
    pub float_history: Vec<FloatStatus>,
    pub absent: bool,
    pub withdrawn: bool,
    pub forfeit_round: Option<u8>,
}

impl SwissPlayer {
    pub fn new(id: impl Into<String>, rating: u32) -> Self {
        Self {
            id: id.into(),
            rating,
            score: 0.0,
            color_history: Vec::new(),
            opponents: Vec::new(),
            bye_rounds: Vec::new(),
            float_history: Vec::new(),
            absent: false,
            withdrawn: false,
            forfeit_round: None,
        }
    }

    pub fn bye_count(&self) -> usize {
        self.bye_rounds.len()
    }

    pub fn had_bye_in(&self, round: u8) -> bool {
        self.bye_rounds.contains(&round)
    }

    pub fn last_float(&self) -> FloatStatus {
        self.float_history
            .last()
            .copied()
            .unwrap_or(FloatStatus::None)
    }

    pub fn floated_down_last_round(&self) -> bool {
        matches!(self.last_float(), FloatStatus::Down)
    }

    pub fn floated_up_last_round(&self) -> bool {
        matches!(self.last_float(), FloatStatus::Up)
    }

    pub fn color_balance(&self) -> i8 {
        let whites = self
            .color_history
            .iter()
            .filter(|c| **c == Color::White)
            .count() as i8;
        let blacks = self
            .color_history
            .iter()
            .filter(|c| **c == Color::Black)
            .count() as i8;
        blacks - whites
    }

    pub fn had_same_color_twice(&self) -> bool {
        if self.color_history.len() < 2 {
            return false;
        }
        let len = self.color_history.len();
        self.color_history[len - 1] == self.color_history[len - 2]
    }

    pub fn would_violate_three_in_row(&self, color: Color) -> bool {
        if self.color_history.len() < 2 {
            return false;
        }
        let len = self.color_history.len();
        if self.color_history[len - 1] == color && self.color_history[len - 2] == color {
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn opposite(&self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum FloatStatus {
    #[default]
    None,
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pairing {
    pub white: String,
    pub black: String,
    pub board: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwissRound {
    pub round: u8,
    pub pairings: Vec<Pairing>,
    pub byes: Vec<String>,
    #[serde(default)]
    pub float_downs: Vec<String>,
    #[serde(default)]
    pub float_ups: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchResult {
    WhiteWin,
    BlackWin,
    Draw,
    Bye,
    ForfeitWhiteWin,
    ForfeitBlackWin,
}

impl MatchResult {
    pub fn white_score(&self) -> f64 {
        match self {
            MatchResult::WhiteWin | MatchResult::ForfeitWhiteWin | MatchResult::Bye => 1.0,
            MatchResult::Draw => 0.5,
            MatchResult::BlackWin | MatchResult::ForfeitBlackWin => 0.0,
        }
    }

    pub fn black_score(&self) -> f64 {
        match self {
            MatchResult::BlackWin | MatchResult::ForfeitBlackWin => 1.0,
            MatchResult::Draw => 0.5,
            MatchResult::WhiteWin | MatchResult::ForfeitWhiteWin | MatchResult::Bye => 0.0,
        }
    }

    pub fn is_forfeit(&self) -> bool {
        matches!(
            self,
            MatchResult::ForfeitWhiteWin | MatchResult::ForfeitBlackWin
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TournamentFormat {
    SingleElimination,
    Swiss { rounds: u8 },
}

#[derive(Debug, Clone)]
pub struct Scoregroup {
    pub score: f64,
    pub players: Vec<SwissPlayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StandingsEntry {
    pub player_id: String,
    pub score: f64,
    pub buchholz: f64,
    pub sonneborn: f64,
    pub rating: u32,
    pub rank: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PairingConfig {
    pub forbidden: Vec<(String, String)>,
    pub manual_overrides: Vec<ManualPairing>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManualPairing {
    pub white: String,
    pub black: String,
}

impl PairingConfig {
    pub fn is_forbidden(&self, a: &str, b: &str) -> bool {
        self.forbidden.iter().any(|(x, y)| {
            (x.as_str() == a && y.as_str() == b) || (x.as_str() == b && y.as_str() == a)
        })
    }
}
