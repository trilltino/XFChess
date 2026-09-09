use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct PlayerSession {
    pub player: Pubkey,
    pub session_key: Pubkey,
    pub expires_at: i64,
    pub spending_limit: u64,
    pub total_spent: u64,
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
    pub const DEFAULT_DURATION: i64 = 86_400; // 24 hours
    pub const DEFAULT_SPENDING_LIMIT: u64 = 500_000_000; // 0.5 SOL
    pub const MAX_WAGER_DEFAULT: u64 = 10_000_000_000; // 10 SOL

    pub fn is_valid(&self, now: i64) -> bool {
        self.is_active && now < self.expires_at
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
