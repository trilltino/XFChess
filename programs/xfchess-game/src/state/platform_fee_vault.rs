//! Defines the global treasury state holding collected platform fees.

use anchor_lang::prelude::*;

/// Platform fee vault — accumulates 0.05 SOL per ranked game.
/// Anyone can trigger `claim_fees` once the threshold or interval is met.
#[account]
#[derive(InitSpace)]
pub struct PlatformFeeVault {
    /// The wallet that receives claimed fees.
    pub host_wallet: Pubkey,
    /// Total lamports accumulated since last claim.
    pub total_accumulated: u64,
    /// Lamports threshold to trigger auto-claim (default: 100_000_000 = 0.1 SOL).
    pub auto_claim_threshold: u64,
    /// Seconds between forced claims even below threshold (default: 86400 = 24h).
    pub claim_interval_seconds: i64,
    pub last_claim_at: i64,
    pub total_claimed: u64,
    pub bump: u8,
}

impl PlatformFeeVault {
    pub const SEED: &'static [u8] = b"platform_fee_vault";
    pub const DEFAULT_THRESHOLD: u64 = 100_000_000;   // 0.1 SOL
    pub const DEFAULT_INTERVAL: i64 = 86_400;          // 24 hours
    pub const FEE_PER_GAME: u64 = 50_000_000;          // 0.05 SOL per ranked game

    /// Returns true if a claim should be triggered.
    pub fn should_claim(&self, now: i64) -> bool {
        self.total_accumulated >= self.auto_claim_threshold
            || (now - self.last_claim_at) >= self.claim_interval_seconds
    }
}
