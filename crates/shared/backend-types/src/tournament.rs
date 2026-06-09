use serde::{Deserialize, Serialize};

/// Tournament summary for listing.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub max_players: u16,
    pub registered: usize,
    pub status: String,
    #[serde(default)]
    pub is_private: bool,
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
