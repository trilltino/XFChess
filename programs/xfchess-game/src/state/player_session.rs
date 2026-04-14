//! Account tracking the state and active keys for a game session.

use anchor_lang::prelude::*;

/// Per-player session delegation.
/// Created by the player once per day: allows a session keypair to sign
/// game transactions without a wallet popup for 24 hours.
#[account]
#[derive(InitSpace)]
pub struct PlayerSession {
    /// The owner's main wallet.
    pub player: Pubkey,
    /// The delegated keypair's public key (stored on the VPS).
    pub session_key: Pubkey,
    /// Unix timestamp when this session expires.
    pub expires_at: i64,
    /// Max lamports the session key may spend in total.
    pub spending_limit: u64,
    /// Lamports spent so far this session.
    pub total_spent: u64,
    /// Max lamports per individual game wager.
    pub max_wager: u64,
    pub can_create_games: bool,
    pub can_join_games: bool,
    pub can_claim_prizes: bool,
    pub games_played: u32,
    pub is_active: bool,
    pub bump: u8,
}

impl PlayerSession {
    pub const SEED: &'static [u8] = b"player_session";
    pub const DEFAULT_DURATION: i64 = 86_400;              // 24 hours
    pub const DEFAULT_SPENDING_LIMIT: u64 = 500_000_000;   // 0.5 SOL
    pub const MAX_WAGER_DEFAULT: u64 = 10_000_000_000;     // 10 SOL

    pub fn is_valid(&self, now: i64) -> bool {
        self.is_active && now < self.expires_at
    }

    pub fn has_budget(&self, amount: u64) -> bool {
        self.total_spent.saturating_add(amount) <= self.spending_limit
            && amount <= self.max_wager
    }
}
