use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct TournamentMatch {
    pub tournament_id: u64,
    pub match_index: u16,
    pub round: u8,
    pub player_white: Option<Pubkey>,
    pub player_black: Option<Pubkey>,
    pub winner: Option<Pubkey>,
    pub game_pda: Option<Pubkey>,
    pub game_id: Option<u64>,
    pub status: MatchStatus,
    pub next_match_for_winner: Option<u16>,
    pub next_match_slot: u8,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum MatchStatus {
    Pending,   // Players assigned, game not yet started
    Active,    // Game PDA created, in progress
    Completed, // Winner recorded
    Bye,       // Player advances without playing (unused in 4-player, reserved)
}
