use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct DisputeRecord {
    pub game_id: u64,
    pub challenger: Pubkey,
    #[max_len(200)]
    pub reason: String,
    pub evidence_hash: [u8; 32],
    pub status: DisputeStatus,
    pub resolved_by: Option<Pubkey>,
    #[max_len(200)]
    pub resolution: String,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum DisputeStatus {
    Pending,
    Resolved,
    Dismissed,
}
