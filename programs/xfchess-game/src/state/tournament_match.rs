//! State definitions representing a single match bracket within a tournament.
//! Supports dynamic single-elimination brackets of any size.

use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct TournamentMatch {
    pub tournament_id: u64,
    /// Match index within the tournament (0 to total_matches-1).
    pub match_index: u16,
    /// Round number (0 = first round, 1 = second round, etc.).
    pub round: u8,
    pub player_white: Option<Pubkey>,
    pub player_black: Option<Pubkey>,
    pub winner: Option<Pubkey>,
    /// On-chain Game PDA for this match (set when the match is started).
    pub game_pda: Option<Pubkey>,
    pub game_id: Option<u64>,
    pub status: MatchStatus,
    /// Index of the next match the winner advances to (None for final).
    pub next_match_for_winner: Option<u16>,
    /// Slot in the next match (0 = white, 1 = black).
    pub next_match_slot: u8,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum MatchStatus {
    Pending,    // Players assigned, game not yet started
    Active,     // Game PDA created, in progress
    Completed,  // Winner recorded
    Bye,        // Player advances without playing (unused in 4-player, reserved)
}
