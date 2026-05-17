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
}
