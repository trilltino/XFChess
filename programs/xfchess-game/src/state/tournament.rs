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
    /// Registered players, fixed 4-slot array. Default pubkey = empty slot.
    pub players: [Pubkey; 4],
    pub registered_count: u8,
    pub status: TournamentStatus,
    pub current_round: u8,
    /// PDAs of the three TournamentMatch accounts (SF1, SF2, Final).
    pub semi_final_1: Pubkey,
    pub semi_final_2: Pubkey,
    pub final_match: Pubkey,
    pub winner: Option<Pubkey>,
    /// ELO ratings mirrored at registration time for bracket seeding.
    pub player_elos: [u32; 4],
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
