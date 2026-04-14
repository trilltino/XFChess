//! State structure defining tournament meta-info, prize pools, and progression.
//! Supports 8, 16, 32, 64, 128 player single-elimination tournaments.

use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Tournament {
    pub tournament_id: u64,
    pub authority: Pubkey,
    #[max_len(64)]
    pub name: String,
    pub entry_fee: u64,
    pub prize_pool: u64,
    /// Maximum players (must be power of 2: 8, 16, 32, 64, 128).
    pub max_players: u16,
    /// Current number of registered players.
    pub registered_count: u16,
    pub status: TournamentStatus,
    pub current_round: u8,
    /// Total number of matches in the tournament (max_players - 1).
    pub total_matches: u16,
    /// Index of the final match (always total_matches - 1).
    pub final_match_index: u16,
    /// Tournament winner (set when final completes).
    pub winner: Option<Pubkey>,
    /// Second place (final loser).
    pub second_place: Option<Pubkey>,
    /// Third place (semifinal loser with higher ELO or first semifinal).
    pub third_place: Option<Pubkey>,
    /// Fourth place (other semifinal loser).
    pub fourth_place: Option<Pubkey>,
    /// Prize distribution: [1st%, 2nd%, 3rd%, 4th%] in basis points (e.g., 5000 = 50%).
    /// Default for 16+ players: [5000, 3000, 1500, 500] = 50/30/15/5%
    /// For 8 players: [10000, 0, 0, 0] = winner-take-all
    pub prize_shares: [u16; 4],
    /// Registered players (up to 128).
    #[max_len(128)]
    pub players: Vec<Pubkey>,
    /// ELO ratings mirrored at registration time for bracket seeding.
    #[max_len(128)]
    pub player_elos: Vec<u32>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum TournamentStatus {
    Registration,
    Active,
    Completed,
    Cancelled,
}
