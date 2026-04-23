//! PDA calculation for Solana Blinks tournament registration.
//!
//! This module provides functions to derive all required PDAs for the
//! RegisterPlayer instruction, ensuring exact match with the smart contract
//! to avoid transaction failures.

use anyhow::Result;
use solana_sdk::pubkey::Pubkey;

use super::{
    PROFILE_SEED, TOURNAMENT_ESCROW_SEED, TOURNAMENT_OPERATOR_ESCROW_SEED,
    TOURNAMENT_OPS_ESCROW_SEED, TOURNAMENT_PRIZE_ESCROW_SEED, TOURNAMENT_SEED,
    TOURNAMENT_USDC_PRIZE_SEED,
};

/// Derives the tournament PDA address.
///
/// Seeds: ["tournament", tournament_id_le_bytes]
pub fn derive_tournament_pda(tournament_id: u64, program_id: &Pubkey) -> Result<Pubkey> {
    let seeds = &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()];
    let (pda, _bump) = Pubkey::find_program_address(seeds, program_id);
    Ok(pda)
}

/// Derives the tournament escrow PDA address.
///
/// Seeds: ["tournament_escrow", tournament_id_le_bytes]
pub fn derive_escrow_pda(tournament_id: u64, program_id: &Pubkey) -> Result<Pubkey> {
    let seeds = &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()];
    let (pda, _bump) = Pubkey::find_program_address(seeds, program_id);
    Ok(pda)
}

/// Derives the tournament prize escrow PDA address.
///
/// Seeds: ["tournament_prize_escrow", tournament_id_le_bytes]
pub fn derive_prize_escrow_pda(tournament_id: u64, program_id: &Pubkey) -> Result<Pubkey> {
    let seeds = &[TOURNAMENT_PRIZE_ESCROW_SEED, &tournament_id.to_le_bytes()];
    let (pda, _bump) = Pubkey::find_program_address(seeds, program_id);
    Ok(pda)
}

/// Derives the tournament ops escrow PDA address.
///
/// Seeds: ["tournament_ops_escrow", tournament_id_le_bytes]
pub fn derive_ops_escrow_pda(tournament_id: u64, program_id: &Pubkey) -> Result<Pubkey> {
    let seeds = &[TOURNAMENT_OPS_ESCROW_SEED, &tournament_id.to_le_bytes()];
    let (pda, _bump) = Pubkey::find_program_address(seeds, program_id);
    Ok(pda)
}

/// Derives the tournament operator escrow PDA address.
///
/// Seeds: ["tournament_operator_escrow", tournament_id_le_bytes]
pub fn derive_operator_escrow_pda(tournament_id: u64, program_id: &Pubkey) -> Result<Pubkey> {
    let seeds = &[TOURNAMENT_OPERATOR_ESCROW_SEED, &tournament_id.to_le_bytes()];
    let (pda, _bump) = Pubkey::find_program_address(seeds, program_id);
    Ok(pda)
}

/// Derives the USDC prize escrow PDA address.
///
/// Seeds: ["t_usdc_prize", tournament_id_le_bytes]
pub fn derive_usdc_prize_escrow_pda(tournament_id: u64, program_id: &Pubkey) -> Result<Pubkey> {
    let seeds = &[TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()];
    let (pda, _bump) = Pubkey::find_program_address(seeds, program_id);
    Ok(pda)
}

/// Derives the player profile PDA address.
///
/// Seeds: ["profile", wallet_pubkey_bytes]
pub fn derive_player_profile_pda(wallet_pubkey: &Pubkey, program_id: &Pubkey) -> Result<Pubkey> {
    let seeds = &[PROFILE_SEED, wallet_pubkey.as_ref()];
    let (pda, _bump) = Pubkey::find_program_address(seeds, program_id);
    Ok(pda)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::Keypair;
    use solana_sdk::signer::Signer;
    use std::str::FromStr;

    #[test]
    fn test_pda_derivation() {
        let program_id = Pubkey::from_str("A5HtSnmyTPohayj9633D9queFFmL2ep6u45nv1v4Wj3W").unwrap();
        let tournament_id = 1u64;

        let tournament_pda = derive_tournament_pda(tournament_id, &program_id).unwrap();
        assert_ne!(tournament_pda, program_id);

        let escrow_pda = derive_escrow_pda(tournament_id, &program_id).unwrap();
        assert_ne!(escrow_pda, program_id);
        assert_ne!(escrow_pda, tournament_pda);

        let wallet_pubkey = Keypair::new().pubkey();
        let profile_pda = derive_player_profile_pda(&wallet_pubkey, &program_id).unwrap();
        assert_ne!(profile_pda, program_id);
        assert_ne!(profile_pda, wallet_pubkey);
    }
}
