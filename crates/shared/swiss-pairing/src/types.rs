use serde::{Deserialize, Serialize};

/// A player in a Swiss tournament
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwissPlayer {
    /// Unique player identifier (e.g., wallet pubkey)
    pub id: String,
    /// ELO rating
    pub rating: u32,
    /// Current score (1.0 for win, 0.5 for draw, 0.0 for loss)
    pub score: f64,
    /// Color history - order of colors played
    pub color_history: Vec<Color>,
    /// IDs of previously played opponents
    pub opponents: Vec<String>,
    /// Rounds in which this player received a bye
    pub bye_rounds: Vec<u8>,
    /// Float history per round (one entry per round played)
    pub float_history: Vec<FloatStatus>,
    /// True = skip from pairing this round; still counts for Buchholz
    pub absent: bool,
    /// Permanently removed; excluded from future rounds AND Buchholz
    pub withdrawn: bool,
    /// Round in which this player forfeited (for reporting)
    pub forfeit_round: Option<u8>,
}

impl SwissPlayer {
    /// Create a new player with default values
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

    /// Number of byes received
    pub fn bye_count(&self) -> usize {
        self.bye_rounds.len()
    }

    /// Whether this player received a bye in the given round
    pub fn had_bye_in(&self, round: u8) -> bool {
        self.bye_rounds.contains(&round)
    }

    /// Most recent float status (None if no rounds played)
    pub fn last_float(&self) -> FloatStatus {
        self.float_history
            .last()
            .copied()
            .unwrap_or(FloatStatus::None)
    }

    /// True if the player was floated down in the immediately preceding round
    pub fn floated_down_last_round(&self) -> bool {
        matches!(self.last_float(), FloatStatus::Down)
    }

    /// True if the player was floated up in the immediately preceding round
    pub fn floated_up_last_round(&self) -> bool {
        matches!(self.last_float(), FloatStatus::Up)
    }

    /// Calculate color balance (positive = needs white, negative = needs black)
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

    /// Check if player had same color twice in a row
    pub fn had_same_color_twice(&self) -> bool {
        if self.color_history.len() < 2 {
            return false;
        }
        let len = self.color_history.len();
        self.color_history[len - 1] == self.color_history[len - 2]
    }

    /// Check if assigning a color would violate 3-in-a-row rule
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

/// Chess colors
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Color {
    /// White pieces.
    White,
    /// Black pieces.
    Black,
}

impl Color {
    /// Get the opposite color
    pub fn opposite(&self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

/// Float status for pairing across scoregroups
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum FloatStatus {
    /// Paired within the player's own scoregroup; no float.
    #[default]
    None,
    /// Floated up from a lower scoregroup.
    Up,
    /// Floated down from a higher scoregroup.
    Down,
}

/// A pairing between two players
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pairing {
    /// White player ID
    pub white: String,
    /// Black player ID
    pub black: String,
    /// Board number for display
    pub board: u16,
}

/// A complete round of pairings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwissRound {
    /// Round number (1-indexed)
    pub round: u8,
    /// All pairings for this round
    pub pairings: Vec<Pairing>,
    /// Players receiving a bye this round
    pub byes: Vec<String>,
    /// Players who floated down (could not be paired within their scoregroup)
    #[serde(default)]
    pub float_downs: Vec<String>,
    /// Players who floated up (paired from a lower scoregroup into a higher one)
    #[serde(default)]
    pub float_ups: Vec<String>,
}

/// Match result
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchResult {
    /// White won on the board.
    WhiteWin,
    /// Black won on the board.
    BlackWin,
    /// Game was drawn.
    Draw,
    /// Player received a bye (no opponent this round); scores as a win.
    Bye,
    /// Black player no-showed; white wins by forfeit
    ForfeitWhiteWin,
    /// White player no-showed; black wins by forfeit
    ForfeitBlackWin,
}

impl MatchResult {
    /// Get score for white player
    pub fn white_score(&self) -> f64 {
        match self {
            MatchResult::WhiteWin | MatchResult::ForfeitWhiteWin | MatchResult::Bye => 1.0,
            MatchResult::Draw => 0.5,
            MatchResult::BlackWin | MatchResult::ForfeitBlackWin => 0.0,
        }
    }

    /// Get score for black player
    pub fn black_score(&self) -> f64 {
        match self {
            MatchResult::BlackWin | MatchResult::ForfeitBlackWin => 1.0,
            MatchResult::Draw => 0.5,
            MatchResult::WhiteWin | MatchResult::ForfeitWhiteWin | MatchResult::Bye => 0.0,
        }
    }

    /// True for the two forfeit variants; false for a played result or a bye.
    pub fn is_forfeit(&self) -> bool {
        matches!(
            self,
            MatchResult::ForfeitWhiteWin | MatchResult::ForfeitBlackWin
        )
    }
}

/// Tournament format
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TournamentFormat {
    /// Single-elimination bracket.
    SingleElimination,
    /// Swiss system over a fixed number of rounds.
    Swiss {
        /// Total rounds in the tournament.
        rounds: u8,
    },
}

/// A scoregroup - players with identical scores
#[derive(Debug, Clone)]
pub struct Scoregroup {
    /// The score shared by every player in this group.
    pub score: f64,
    /// Players at this score, in rank order.
    pub players: Vec<SwissPlayer>,
}

/// Standings entry for tournament results
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StandingsEntry {
    /// Same ID space as `SwissPlayer::id`.
    pub player_id: String,
    /// Total tournament score (wins = 1.0, draws = 0.5).
    pub score: f64,
    /// Buchholz tiebreak: sum of opponents' scores.
    pub buchholz: f64,
    /// Sonneborn-Berger tiebreak: sum of defeated opponents' scores plus half of drawn opponents' scores.
    pub sonneborn: f64,
    /// Player rating, carried through for the display and the rating tiebreak.
    pub rating: u32,
    /// 1-indexed final placement after tiebreaks are applied.
    pub rank: u16,
}

/// Configuration for a pairing round
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PairingConfig {
    /// Player ID pairs that must never be matched against each other
    pub forbidden: Vec<(String, String)>,
    /// Manually forced pairings applied before the Dutch algorithm runs
    pub manual_overrides: Vec<ManualPairing>,
}

/// A manually forced pairing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManualPairing {
    /// Player assigned white.
    pub white: String,
    /// Player assigned black.
    pub black: String,
}

impl PairingConfig {
    /// True if `a` and `b` appear together in `forbidden`, in either order.
    pub fn is_forbidden(&self, a: &str, b: &str) -> bool {
        self.forbidden.iter().any(|(x, y)| {
            (x.as_str() == a && y.as_str() == b) || (x.as_str() == b && y.as_str() == a)
        })
    }
}
