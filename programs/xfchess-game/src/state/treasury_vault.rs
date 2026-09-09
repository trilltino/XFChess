use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct TreasuryVault {
    #[max_len(2)]
    pub country: String, // ISO 3166-1 alpha-2 (e.g., "GB", "BR", "CA", "DE")
    pub authority: Pubkey,    // Authority that can withdraw from vault
    pub total_collected: u64, // Total lamports collected in this vault
    pub bump: u8,             // PDA canonical bump
}
