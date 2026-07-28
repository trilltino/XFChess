//! MagicBlock delegation CPI wrappers.

use crate::constants::{ER_COMMIT_FREQUENCY_MS, GAME_SEED};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::program::invoke_signed;
use ephemeral_rollups_sdk::cpi::{delegate_account, DelegateAccounts, DelegateConfig};
use ephemeral_rollups_sdk::ephem::deprecated::v0::commit_and_undelegate_accounts;
use ephemeral_rollups_sdk::instruction_builder;

/// The `Game` PDA's seed bytes (`game_id` little-endian), reused across
/// delegate and undelegate calls.
pub fn game_seed_bytes(game_id: u64) -> [u8; 8] {
    game_id.to_le_bytes()
}

/// Standard delegation config: commits at `ER_COMMIT_FREQUENCY_MS` and
/// leaves `validator: None` so the magic-router assigns one — pinning a
/// validator here previously broke mainnet (see `delegation_ix::delegate`).
pub fn default_delegate_config() -> DelegateConfig {
    DelegateConfig {
        commit_frequency_ms: ER_COMMIT_FREQUENCY_MS,
        validator: None,
    }
}

/// Delegates a `Game` PDA to the Ephemeral Rollup using the standard config.
pub fn delegate_game_pda<'a, 'info>(
    accounts: DelegateAccounts<'a, 'info>,
    game_id_bytes: &[u8; 8],
) -> Result<()> {
    let seeds: &[&[u8]] = &[b"game", game_id_bytes];
    delegate_account(accounts, seeds, default_delegate_config())?;
    Ok(())
}

/// Commits the ER's current `Game` state to the base layer and undelegates
/// the account so base-layer instructions can act on it again.
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

/// Verifies `infos` are exactly the accounts (same pubkeys, same order)
/// `ix.accounts` expects, so the delegation-program instruction we're about
/// to CPI into runs against the accounts our own program validated — not
/// whatever the caller happened to pass. Avoids re-deriving each PDA a
/// second time (and the `dlp_api` pubkey-compat conversions that would take)
/// by trusting the SDK's own instruction builder as the source of truth for
/// what each account must be.
fn require_accounts_match_instruction(ix: &Instruction, infos: &[AccountInfo]) -> Result<()> {
    require_eq!(
        ix.accounts.len(),
        infos.len(),
        crate::errors::GameErrorCode::InvalidArgument
    );
    for (meta, info) in ix.accounts.iter().zip(infos.iter()) {
        require_keys_eq!(
            meta.pubkey,
            info.key(),
            crate::errors::GameErrorCode::InvalidArgument
        );
    }
    Ok(())
}

/// Requests undelegation of a `Game` PDA without the ER validator's
/// cooperation — the owner-program-authorized escape hatch for a validator
/// that has gone unreachable. Starts the delegation program's
/// `DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS` (~60min) countdown; once it
/// elapses, `force_undelegate_after_timeout` can complete the recovery
/// without the validator at all. See MAGICBLOCK.md's
/// "Failure Mode: ER Unavailability" section — this does **not** preserve
/// the game's data (see that instruction's own doc comment for why).
#[allow(clippy::too_many_arguments)]
pub fn request_force_undelegate<'info>(
    payer: &AccountInfo<'info>,
    game: &AccountInfo<'info>,
    owner_program: &AccountInfo<'info>,
    undelegation_request: &AccountInfo<'info>,
    delegation_record: &AccountInfo<'info>,
    delegation_metadata: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    game_id_bytes: &[u8; 8],
    game_bump: u8,
) -> Result<()> {
    let ix = instruction_builder::request_undelegation(
        payer.key(),
        game.key(),
        owner_program.key(),
    );
    let account_infos = [
        payer.clone(),
        game.clone(),
        owner_program.clone(),
        undelegation_request.clone(),
        delegation_record.clone(),
        delegation_metadata.clone(),
        system_program.clone(),
    ];
    require_accounts_match_instruction(&ix, &account_infos)?;
    let seeds: &[&[u8]] = &[GAME_SEED, game_id_bytes, &[game_bump]];
    invoke_signed(&ix, &account_infos, &[seeds])?;
    Ok(())
}

/// Completes a forced undelegation once the `request_force_undelegate`
/// timeout has elapsed. **Data-loss warning (from the delegation program's
/// own doc comment):** this does not accept or apply any pending validator
/// commit and does not preserve the account's prior data either — the
/// delegation program resizes the account to zero bytes and hands ownership
/// back empty. Callers get the `Game` PDA back under program ownership, not
/// a working `Game` account; recovering the escrow needs a dedicated
/// instruction that doesn't depend on the wiped account's contents (see
/// `governance_ix::recover_stuck_delegation`).
///
/// `commit_reimbursement` only matters if the validator left a pending
/// (uncommitted) commit behind; in the common case (validator never
/// committed at all) it's read but not written to, so any account works —
/// callers may pass `delegation_rent_payer` again as a placeholder.
#[allow(clippy::too_many_arguments)]
pub fn force_undelegate_after_timeout<'info>(
    game: &AccountInfo<'info>,
    owner_program: &AccountInfo<'info>,
    undelegation_request: &AccountInfo<'info>,
    delegation_record: &AccountInfo<'info>,
    delegation_metadata: &AccountInfo<'info>,
    delegation_rent_payer: &AccountInfo<'info>,
    commit_state: &AccountInfo<'info>,
    commit_record: &AccountInfo<'info>,
    commit_reimbursement: &AccountInfo<'info>,
    game_id_bytes: &[u8; 8],
    game_bump: u8,
) -> Result<()> {
    let ix = instruction_builder::undelegate_with_rollback_after_timeout(
        game.key(),
        owner_program.key(),
        delegation_rent_payer.key(),
        commit_reimbursement.key(),
    );
    let account_infos = [
        game.clone(),
        owner_program.clone(),
        undelegation_request.clone(),
        delegation_record.clone(),
        delegation_metadata.clone(),
        delegation_rent_payer.clone(),
        commit_state.clone(),
        commit_record.clone(),
        commit_reimbursement.clone(),
    ];
    require_accounts_match_instruction(&ix, &account_infos)?;
    let seeds: &[&[u8]] = &[GAME_SEED, game_id_bytes, &[game_bump]];
    invoke_signed(&ix, &account_infos, &[seeds])?;
    Ok(())
}
