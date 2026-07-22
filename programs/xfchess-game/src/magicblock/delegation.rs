//! MagicBlock delegation CPI wrappers.

use crate::constants::ER_COMMIT_FREQUENCY_MS;
use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::cpi::{delegate_account, DelegateAccounts, DelegateConfig};
use ephemeral_rollups_sdk::ephem::deprecated::v0::commit_and_undelegate_accounts;

pub fn game_seed_bytes(game_id: u64) -> [u8; 8] {
    game_id.to_le_bytes()
}

pub fn default_delegate_config() -> DelegateConfig {
    DelegateConfig {
        commit_frequency_ms: ER_COMMIT_FREQUENCY_MS,
        validator: None,
    }
}

pub fn delegate_game_pda<'a, 'info>(
    accounts: DelegateAccounts<'a, 'info>,
    game_id_bytes: &[u8; 8],
) -> Result<()> {
    let seeds: &[&[u8]] = &[b"game", game_id_bytes];
    delegate_account(accounts, seeds, default_delegate_config())?;
    Ok(())
}

pub fn commit_and_undelegate_game_pda<'info>(
    payer: &AccountInfo<'info>,
    game: &AccountInfo<'info>,
    magic_context: &AccountInfo<'info>,
    magic_program: &AccountInfo<'info>,
) -> Result<()> {
    commit_and_undelegate_accounts(payer, vec![game], magic_context, magic_program, None)?;
    Ok(())
}

/// Derives the delegation program's canonical undelegate-buffer PDA for a
/// given delegated account: `["undelegate-buffer", delegated_account]` under
/// the delegation program ID (see `dlp-api`'s `undelegate_buffer_pda_from_delegated_account`).
///
/// `ephemeral_rollups_sdk::cpi::undelegate_account` (as of SDK <= 0.16.1) checks
/// that `buffer` is a signer owned by the delegation program, but never that it
/// is *this specific* account's buffer — any delegation-owned signer buffer is
/// accepted. That lets an attacker delegate their own manufactured account
/// (getting a legitimate delegation-owned signer buffer) and substitute it into
/// another account's `process_undelegation` call, overwriting that account's
/// data with attacker-chosen bytes. SDK 0.16.2 added this exact check
/// (MagicBlock advisory, 2026-07-22); we can't take the SDK bump yet (it pulls
/// in `solana-pubkey ^2.4`, which conflicts with the workspace's `solana-client
/// = 2.2.1` pin), so the check is replicated here until that ceiling lifts.
pub fn undelegate_buffer_pda(delegated_account: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"undelegate-buffer", delegated_account.as_ref()],
        &ephemeral_rollups_sdk::id(),
    )
    .0
}
