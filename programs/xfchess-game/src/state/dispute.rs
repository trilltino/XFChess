//! Data models for in-game dispute metadata and evidence.

use anchor_lang::prelude::*;

/// One dispute per game (seed: `[b"dispute", game_id]`), opened by
/// `governance_ix::dispute` and closed out by either `resolve` (authority
/// ruling) or `claim_stale_dispute` (TTL auto-resolution).
#[account]
#[derive(InitSpace)]
pub struct DisputeRecord {
    pub game_id: u64,
    /// The player who opened the dispute.
    pub challenger: Pubkey,
    #[max_len(200)]
    pub reason: String,
    pub evidence_hash: [u8; 32],
    pub status: DisputeStatus,
    pub resolved_by: Option<Pubkey>,
    #[max_len(200)]
    pub resolution: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub resolved_at: Option<i64>,
    /// Lamports the challenger posted as a bond when opening the dispute. Held in
    /// this record's PDA; refunded to the challenger if the dispute is upheld or
    /// auto-resolved after the TTL, forfeited to the treasury if dismissed.
    pub bond_amount: u64,
    pub bump: u8,
}

/// Dispute lifecycle state.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum DisputeStatus {
    /// Open, awaiting either an authority ruling or TTL expiry.
    Pending,
    /// Ruled on by the platform dispute authority (`governance_ix::resolve`).
    Resolved,
    /// Auto-resolved after `DISPUTE_TTL_SECS` with no ruling (`claim_stale_dispute`).
    Dismissed,
}
