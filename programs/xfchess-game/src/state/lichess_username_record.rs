use anchor_lang::prelude::*;

#[account]
pub struct LichessUsernameRecord {
    pub owner: Pubkey,   // Wallet that linked this Lichess username
    pub created_at: i64, // Timestamp when first linked
}

impl LichessUsernameRecord {
    pub const LEN: usize = 8 + 32 + 8; // Discriminator + Pubkey + i64
}
