use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GlobalSessionDelegation {
    pub player: Pubkey,
    pub session_key: Pubkey,
    pub expires_at: i64,
    pub spending_limit: u64,
    pub total_spent: u64,
    pub max_wager: u64,
    pub games_remaining: u16,
    pub enabled: bool,
    pub bump: u8,
}

impl GlobalSessionDelegation {
    pub const SEED: &'static [u8] = b"global_session";
    pub const DEFAULT_DURATION: i64 = 30 * 24 * 60 * 60;
    pub const DEFAULT_GAMES: u16 = 200;
    pub const DEFAULT_SPENDING_LIMIT: u64 = 5_000_000_000;
    pub const DEFAULT_MAX_WAGER: u64 = 1_000_000_000;

    pub fn is_valid(&self, now: i64) -> bool {
        self.enabled && now < self.expires_at && self.games_remaining > 0
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
