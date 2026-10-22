use anchor_lang::prelude::*;

/// Ties one tournament to its pass mint. A separate account keeps the deployed `Tournament`
/// layout unchanged. An empty, system-owned binding address means the tournament needs no pass.
#[account]
#[derive(InitSpace)]
pub struct TournamentPassBinding {
    pub tournament_id: u64,
    pub pass_mint: Pubkey,
    pub bump: u8,
}
