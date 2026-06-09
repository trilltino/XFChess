use serde::{Deserialize, Serialize};

/// Swiss tournament gossip messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwissMessage {
    /// New round pairings available
    RoundStarted {
        tournament_id: u64,
        round: u8,
        pairings: Vec<SwissPairing>,
    },
    /// Match result recorded
    ResultRecorded {
        tournament_id: u64,
        round: u8,
        board: u16,
        result: MatchResult,
    },
    /// Standings updated
    StandingsUpdated {
        tournament_id: u64,
        standings: Vec<SwissStandingsEntry>,
    },
    /// Bracket has fired (async fill or scheduled start).
    /// Clients should fetch GET /tournament/{id}/bracket to find their match.
    BracketFired {
        tournament_id: u64,
        /// Number of players who entered the bracket.
        player_count: u16,
        /// Unix timestamp of the start.
        started_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwissPairing {
    pub white: String,
    pub black: String,
    pub board: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchResult {
    Win { winner: String },
    Draw,
}

impl std::fmt::Display for MatchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchResult::Win { winner } => write!(f, "win:{winner}"),
            MatchResult::Draw => write!(f, "draw"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwissStandingsEntry {
    pub player_id: String,
    pub score: f64,
    pub rank: u16,
}
