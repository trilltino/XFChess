use serde::{Deserialize, Serialize};

/// One row of the tournament listing returned by the backend's tournament routes
/// and rendered by the web frontend's tournament list/detail views.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TournamentSummary {
    /// On-chain tournament account identifier.
    pub tournament_id: u64,
    pub name: String,
    /// Entry fee in lamports; 0 for free tournaments.
    pub entry_fee_lamports: u64,
    /// Total prize pool in lamports (or USDC base units when `usdc_mint` is set).
    pub prize_pool: u64,
    pub max_players: u16,
    /// Current number of registered players.
    pub registered: usize,
    /// Tournament lifecycle state as a string (e.g. `"registration"`, `"active"`, `"finished"`).
    pub status: String,
    #[serde(default)]
    pub is_private: bool,
    /// True for bracket/Swiss tournaments; false for casual matchmaking listings that reuse this shape.
    #[serde(default)]
    pub is_tournament: bool,
    /// Minimum ELO required to register (0 = no requirement).
    #[serde(default)]
    pub min_elo: u16,
    /// Maximum ELO allowed (0 = no cap).
    #[serde(default)]
    pub max_elo: u16,
    /// SPL mint address when prize is paid in USDC; None = SOL prize.
    #[serde(default)]
    pub usdc_mint: Option<String>,
    /// Unix timestamp (seconds) when the current round's time limit expires.
    #[serde(default)]
    pub round_deadline_at: Option<u64>,
}
