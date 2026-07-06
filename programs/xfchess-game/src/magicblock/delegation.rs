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
