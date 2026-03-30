use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct PlayerProfile {
    pub authority: Pubkey,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub games_played: u32,
    pub elo: u16,
}
