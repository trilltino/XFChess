//! On-chain friendship edge between two players (Solana Friends).
//!
//! One PDA per *undirected* pair, addressed by the two wallet pubkeys in
//! canonical (sorted) order — seeds `["friendship", lo, hi]` — so both sides
//! derive the same account regardless of who asks. See `account_ix/friends_ix.rs`.

use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Friendship {
    /// The player who sent the request.
    pub requester: Pubkey,
    /// The player who must accept (the other party).
    pub addressee: Pubkey,
    /// Current edge status.
    pub status: FriendStatus,
    /// Unix timestamp the request was created.
    pub created_at: i64,
    /// Unix timestamp the request was accepted (0 until accepted).
    pub accepted_at: i64,
    /// PDA bump.
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum FriendStatus {
    Pending,
    Accepted,
    Blocked,
}
