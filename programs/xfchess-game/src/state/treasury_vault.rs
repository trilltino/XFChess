//! Typed regional-treasury account definition. Not currently used — see the
//! doc comment on `TreasuryVault` below.

use anchor_lang::prelude::*;

/// A per-country treasury account shape (never instantiated). Every actual
/// treasury reference in the program (`account_ix::treasury`,
/// `governance_ix::resolve`, `governance_ix::claim_stale_dispute`,
/// `lifecycle::settlement`) uses a single global, untyped `SystemAccount` at
/// `seeds = [TREASURY_VAULT_SEED]` — no country code, and not this struct.
/// Confirmed via search: nothing in the program constructs or deserializes
/// a `TreasuryVault`.
#[account]
#[derive(InitSpace)]
pub struct TreasuryVault {
    #[max_len(2)]
    pub country: String, // ISO 3166-1 alpha-2 (e.g., "GB", "BR", "CA", "DE")
    pub authority: Pubkey,    // Authority that can withdraw from vault
    pub total_collected: u64, // Total lamports collected in this vault
    pub bump: u8,             // PDA canonical bump
}
