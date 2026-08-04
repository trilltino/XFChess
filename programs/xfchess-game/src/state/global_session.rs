//! Global persistent session delegation.
//!
//! A [`GlobalSessionDelegation`] allows a hot session keypair to sign every
//! [`CreateGame`](crate::game_ix::CreateGame) and
//! [`JoinGame`](crate::game_ix::JoinGame) instruction on behalf of a player
//! **indefinitely** — no wallet popup per game, no per-tournament setup.
//!
//! The account is keyed solely by `player` and is created once via
//! `authorize_global_session`. It expires after [`DEFAULT_DURATION`] (30 days)
//! from authorization and can be revoked at any time via `revoke_global_session`.
//!
//! # Seeds
//! `[b"global_session", player.as_ref()]`
//!
//! # Security
//! * `spending_limit` caps total lamports the session key can spend so a
//!   stolen key cannot drain the vault.
//! * `max_wager` caps any single game's escrow contribution.
//! * `games_remaining` limits how many games the key may create/join before
//!   requiring a fresh authorization (default 200).
//! * `enabled` can be flipped to `false` by the player via
//!   `revoke_global_session` to kill the key immediately.

use anchor_lang::prelude::*;

/// Global persistent session delegation account.
#[account]
#[derive(InitSpace)]
pub struct GlobalSessionDelegation {
    /// The real wallet this delegation is for.
    pub player: Pubkey,
    /// Hot key held by the VPS / client that may co-sign game ixs.
    pub session_key: Pubkey,
    /// Unix timestamp after which the session key is rejected.
    pub expires_at: i64,
    /// Max lamports the session key may spend over its lifetime.
    pub spending_limit: u64,
    /// Lamports already consumed by this session (escrows, fees, …).
    pub total_spent: u64,
    /// Max lamports permitted for any single game wager.
    pub max_wager: u64,
    /// Games that can still be played before re-auth is required.
    pub games_remaining: u16,
    /// Flipped to `false` by `revoke_global_session`.
    pub enabled: bool,
    /// Canonical PDA bump stored for CPI sign calls.
    pub bump: u8,
}

impl GlobalSessionDelegation {
    /// PDA seed prefix.
    pub const SEED: &'static [u8] = b"global_session";
    /// Default lifetime: 30 days from authorization.
    pub const DEFAULT_DURATION: i64 = 30 * 24 * 60 * 60;
    /// Default game budget before re-auth (200 games ≈ a few tournaments).
    pub const DEFAULT_GAMES: u16 = 200;
    /// Default spending cap: 5 SOL.
    pub const DEFAULT_SPENDING_LIMIT: u64 = 5_000_000_000;
    /// Default per-game wager cap: 1 SOL.
    pub const DEFAULT_MAX_WAGER: u64 = 1_000_000_000;

    /// Returns `true` when the delegation is enabled, not expired, and has
    /// games remaining.
    pub fn is_valid(&self, now: i64) -> bool {
        self.enabled && now < self.expires_at && self.games_remaining > 0
    }

    /// Returns `true` when an additional `amount` spend stays within both
    /// the per-wager and the cumulative caps.
    pub fn has_budget(&self, amount: u64) -> bool {
        amount <= self.max_wager
            && self
                .total_spent
                .checked_add(amount)
                .map(|total| total <= self.spending_limit)
                .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(now: i64) -> GlobalSessionDelegation {
        GlobalSessionDelegation {
            player: Pubkey::new_unique(),
            session_key: Pubkey::new_unique(),
            expires_at: now + 3600,
            spending_limit: 5_000_000_000,
            total_spent: 0,
            max_wager: 1_000_000_000,
            games_remaining: 200,
            enabled: true,
            bump: 255,
        }
    }

    #[test]
    fn is_valid_returns_true_when_enabled_and_future() {
        let s = sample(1000);
        assert!(s.is_valid(1500));
    }

    #[test]
    fn is_valid_returns_false_when_expired() {
        let s = sample(1000);
        assert!(!s.is_valid(99_999));
    }

    #[test]
    fn is_valid_returns_false_when_disabled() {
        let mut s = sample(1000);
        s.enabled = false;
        assert!(!s.is_valid(1500));
    }

    #[test]
    fn is_valid_returns_false_when_no_games_remaining() {
        let mut s = sample(1000);
        s.games_remaining = 0;
        assert!(!s.is_valid(1500));
    }

    #[test]
    fn has_budget_respects_max_wager() {
        let s = sample(1000);
        assert!(s.has_budget(1_000_000_000));
        assert!(!s.has_budget(1_000_000_001));
    }

    #[test]
    fn has_budget_respects_cumulative_cap() {
        let mut s = sample(1000);
        s.total_spent = 4_500_000_000;
        assert!(s.has_budget(500_000_000));
        assert!(!s.has_budget(500_000_001));
    }

    #[test]
    fn has_budget_is_overflow_safe() {
        let mut s = sample(1000);
        s.total_spent = u64::MAX;
        s.max_wager = u64::MAX;
        assert!(!s.has_budget(1));
    }
}
