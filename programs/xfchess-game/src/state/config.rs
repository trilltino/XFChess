use anchor_lang::prelude::*;

#[account]
#[derive(Debug)]
pub struct ProgramConfig {
    /// Super-admin authority that can update this config.
    pub authority: Pubkey,
    /// Recipient of platform fees and host treasury.
    pub treasury_authority: Pubkey,
    /// Automated crank / time check authority.
    pub vps_authority: Pubkey,
    /// Arbiter for resolving disputed games.
    pub dispute_authority: Pubkey,
    /// Signer for external rating / ELO link proofs.
    pub link_authority: Pubkey,
    /// KYC / CACF verification authority.
    pub kyc_authority: Pubkey,

    /// Maximum allowed wager in lamports (e.g. 10 SOL).
    pub max_wager_amount: u64,
    /// Minimum allowed wager in lamports (e.g. 0.001 SOL).
    pub min_wager_lamports: u64,
    /// Maximum platform fee in lamports (e.g. 1 SOL).
    pub max_platform_fee_lamports: u64,
    /// MagicBlock / Ephemeral Rollup session fee in lamports.
    pub er_session_fee_lamports: u64,
    /// Bond required to initiate a game dispute in lamports.
    pub dispute_bond_lamports: u64,
    /// Time-to-live for open disputes before expiration in seconds.
    pub dispute_ttl_secs: i64,
    /// Maximum slot lag tolerated for crank time checks.
    pub crank_max_slot_delay: u64,
    /// Allowed clock jitter for early crank triggers in seconds.
    pub crank_max_seconds_early: i64,

    /// Bump seed for PDA derivation.
    pub bump: u8,
    /// Reserved space for zero-migration additions.
    pub reserved: [u8; 64],
}

impl ProgramConfig {
    pub const LEN: usize = 8 // discriminator
        + 32 * 6             // 6 pubkeys
        + 8 * 8              // 8 u64/i64 parameters
        + 1                  // bump
        + 64; // reserved
}
