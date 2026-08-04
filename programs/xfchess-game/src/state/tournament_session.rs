//! Tournament-scoped session delegation.
//!
//! A [`TournamentSessionDelegation`] allows a hot session keypair to sign
//! every [`CreateGame`](crate::game_ix::CreateGame),
//! [`JoinGame`](crate::game_ix::JoinGame) and
//! [`RecordSwissResult`](crate::tournament_ix::RecordSwissResult) instruction
//! on behalf of a player for a single tournament, without triggering a wallet
//! popup per match.
//!
//! Compared to the per-game [`SessionDelegation`](crate::state::SessionDelegation)
//! which covers one `game_id`, this account is keyed by `(tournament_id,
//! player)` and is created once when the player registers. It expires when
//! the tournament completes or after [`DEFAULT_DURATION`] (48h), whichever
//! comes first.
//!
//! # Seeds
//! `[b"tournament_session", tournament_id.to_le_bytes(), player.as_ref()]`
//!
//! # Security
//! * `spending_limit` caps total lamports the session key can spend across the
//!   tournament so a stolen key cannot drain the wallet.
//! * `max_wager` caps any single game's escrow.
//! * `enabled` can be flipped by the player via
//!   [`revoke_tournament_session`] to kill the key early.
//!
//! Reference: Solana session keys pattern —
//! <https://book.anchor-lang.com/chapter_3/how_it_works.html>.

use anchor_lang::prelude::*;

/// Tournament-scoped session delegation account.
#[account]
#[derive(InitSpace)]
pub struct TournamentSessionDelegation {
    /// Tournament this delegation is bound to.
    pub tournament_id: u64,
    /// The real wallet this delegation is for.
    pub player: Pubkey,
    /// Hot key held by the VPS / client that may co-sign tournament ixs.
    pub session_key: Pubkey,
    /// Unix timestamp after which the session key is rejected.
    pub expires_at: i64,
    /// Max lamports the session key may spend over the tournament's lifetime.
    pub spending_limit: u64,
    /// Lamports already consumed by this session (escrows, fees, …).
    pub total_spent: u64,
    /// Max lamports permitted for any single game wager.
    pub max_wager: u64,
    /// How many games the session key has played so far (monitoring / UI).
    pub games_played: u32,
    /// Flipped to `false` by `revoke_tournament_session`.
    pub enabled: bool,
    /// Canonical PDA bump stored for CPI sign calls.
    pub bump: u8,
}

impl TournamentSessionDelegation {
    /// PDA seed prefix.
    pub const SEED: &'static [u8] = b"tournament_session";
    /// Default lifetime: 48 hours from authorization.
    pub const DEFAULT_DURATION: i64 = 48 * 60 * 60;
    /// Default tournament-wide spending cap: 1 SOL.
    pub const DEFAULT_SPENDING_LIMIT: u64 = 1_000_000_000;
    /// Default per-game wager cap: 0.25 SOL.
    pub const DEFAULT_MAX_WAGER: u64 = 250_000_000;

    /// Returns `true` when the delegation is still enabled and not expired.
    pub fn is_valid(&self, now: i64) -> bool {
        self.enabled && now < self.expires_at
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

    fn sample(now: i64) -> TournamentSessionDelegation {
        TournamentSessionDelegation {
            tournament_id: 1,
            player: Pubkey::new_unique(),
            session_key: Pubkey::new_unique(),
            expires_at: now + 3600,
            spending_limit: 1_000_000_000,
            total_spent: 0,
            max_wager: 250_000_000,
            games_played: 0,
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
    fn has_budget_respects_max_wager() {
        let s = sample(1000);
        assert!(s.has_budget(250_000_000));
        assert!(!s.has_budget(250_000_001));
    }

    #[test]
    fn has_budget_respects_cumulative_cap() {
        let mut s = sample(1000);
        s.total_spent = 900_000_000;
        assert!(s.has_budget(100_000_000));
        assert!(!s.has_budget(100_000_001));
    }

    #[test]
    fn has_budget_is_overflow_safe() {
        let mut s = sample(1000);
        s.total_spent = u64::MAX;
        s.max_wager = u64::MAX;
        assert!(!s.has_budget(1));
    }

    #[test]
    fn is_valid_boundary_is_exclusive() {
        let s = sample(1000);
        // expires_at == 1000 + 3600; exactly equal is NOT valid.
        assert!(s.is_valid(s.expires_at - 1));
        assert!(!s.is_valid(s.expires_at));
    }

    #[test]
    fn sequential_spending_is_tracked() {
        let mut s = sample(1000);
        assert!(s.has_budget(100_000_000));
        s.total_spent = s.total_spent.saturating_add(100_000_000);
        assert!(s.has_budget(100_000_000));
        s.total_spent = s.total_spent.saturating_add(250_000_000);
        // 350M spent + 250M attempt = 600M <= 1G cap
        assert!(s.has_budget(250_000_000));
        s.total_spent = 900_000_000;
        // per-wager cap still enforced
        assert!(!s.has_budget(s.max_wager + 1));
    }
}
