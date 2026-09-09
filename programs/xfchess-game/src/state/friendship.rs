use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Friendship {
    pub requester: Pubkey,
    pub addressee: Pubkey,
    pub status: FriendStatus,
    pub created_at: i64,
    pub accepted_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum FriendStatus {
    Pending,
    Accepted,
    Blocked,
}
