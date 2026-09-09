use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct TournamentSessionDelegation {
    pub tournament_id: u64,
    pub player: Pubkey,
    pub session_key: Pubkey,
    pub expires_at: i64,
    pub spending_limit: u64,
    pub total_spent: u64,
    pub max_wager: u64,
    pub games_played: u32,
    pub enabled: bool,
    pub bump: u8,
}

impl TournamentSessionDelegation {
    pub const SEED: &'static [u8] = b"tournament_session";
    pub const DEFAULT_DURATION: i64 = 48 * 60 * 60;
    pub const DEFAULT_SPENDING_LIMIT: u64 = 1_000_000_000;
    pub const DEFAULT_MAX_WAGER: u64 = 250_000_000;

    pub fn is_valid(&self, now: i64) -> bool {
        self.enabled && now < self.expires_at
    }

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
