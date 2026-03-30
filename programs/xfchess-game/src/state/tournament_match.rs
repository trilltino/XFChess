use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct TournamentMatch {
    pub tournament_id: u64,
    pub match_index: u8,   // 0 = SF1, 1 = SF2, 2 = Final
    pub round: u8,         // 0 = semi-finals, 1 = final
    pub player_white: Option<Pubkey>,
    pub player_black: Option<Pubkey>,
    pub winner: Option<Pubkey>,
    /// On-chain Game PDA for this match (set when the match is started).
    pub game_pda: Option<Pubkey>,
    pub game_id: Option<u64>,
    pub status: MatchStatus,
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
