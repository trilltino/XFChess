//! Self-serve recovery for a `Game` PDA whose ER validator has gone dark.
//!
//! Normal undelegation (`delegate::handler_undelegate_game`) needs the ER
//! validator to actually process the commit+undelegate CPI. If the assigned
//! validator is unreachable, that path — and everything else that only
//! writes through the ER (`claim_timeout`, the crank idle-checks) — is
//! equally stuck, and MagicBlock's only other forced-undelegate
//! (`undelegate_confined_account`) needs an admin key XFChess doesn't hold.
//!
//! The delegation program also exposes a non-admin, owner-program-authorized
//! escape hatch: `request_undelegation` (callable any time) starts a
//! `DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS` (~60min) countdown, after
//! which `undelegate_with_rollback_after_timeout` hands the account back
//! without the validator's cooperation at all. See MAGICBLOCK.md's
//! "Failure Mode: ER Unavailability" section.
//!
//! These two instructions are the on-chain wiring for that path. See
//! `governance_ix::recover_stuck_delegation` for what happens to the escrow
//! afterward — the delegation program hands the `Game` PDA back wiped to
//! zero bytes, not restored, so a separate instruction is needed to recover
//! the wager rather than the normal `finalize_game` path.

use crate::constants::GAME_SEED;
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

/// Accounts for `request_force_undelegate`. Mirrors the delegation
/// program's `process_request_undelegation` account list exactly —
/// see `magicblock::delegation::request_force_undelegate`.
#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RequestForceUndelegateCtx<'info> {
    /// CHECK: the delegated `Game` PDA — still owned by the delegation
    /// program (that's the scenario this instruction exists for), so it
    /// cannot be a typed `Account<'info, Game>`. Signs via program seeds.
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: AccountInfo<'info>,
    /// Must be the same key that funded the original `delegate_game` call
    /// for this game (the delegation program checks it against
    /// `delegation_metadata.rent_payer`).
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: this program, passed as the delegation's `owner_program`.
    #[account(address = crate::ID @ GameErrorCode::InvalidOwnerProgram)]
    pub owner_program: AccountInfo<'info>,
    /// CHECK: verified against the delegation program's own PDA derivation
    /// in `magicblock::delegation::require_accounts_match_instruction`.
    #[account(mut)]
    pub undelegation_request_pda: AccountInfo<'info>,
    /// CHECK: verified against the delegation program's own PDA derivation.
    pub delegation_record_pda: AccountInfo<'info>,
    /// CHECK: verified against the delegation program's own PDA derivation.
    #[account(mut)]
    pub delegation_metadata_pda: AccountInfo<'info>,
    /// CHECK: the delegation program itself.
    #[account(address = ephemeral_rollups_sdk::id())]
    pub delegation_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_request_force_undelegate(
    ctx: Context<RequestForceUndelegateCtx>,
    game_id: u64,
) -> Result<()> {
    let game_id_bytes = game_id.to_le_bytes();
    let game_bump = ctx.bumps.game;
    crate::magicblock::delegation::request_force_undelegate(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.game.to_account_info(),
        &ctx.accounts.owner_program.to_account_info(),
        &ctx.accounts.undelegation_request_pda.to_account_info(),
        &ctx.accounts.delegation_record_pda.to_account_info(),
        &ctx.accounts.delegation_metadata_pda.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &game_id_bytes,
        game_bump,
    )
}

/// Accounts for `force_undelegate_after_timeout`. Mirrors the delegation
/// program's `process_undelegate_with_rollback_after_timeout` account list
/// exactly — see `magicblock::delegation::force_undelegate_after_timeout`.
#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ForceUndelegateAfterTimeoutCtx<'info> {
    /// CHECK: the delegated `Game` PDA, restored to program ownership (wiped
    /// to zero bytes) by this instruction. Signs via program seeds.
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: AccountInfo<'info>,
    /// CHECK: this program, passed as the delegation's `owner_program`.
    #[account(address = crate::ID @ GameErrorCode::InvalidOwnerProgram)]
    pub owner_program: AccountInfo<'info>,
    /// CHECK: verified against the delegation program's own PDA derivation.
    #[account(mut)]
    pub undelegation_request_pda: AccountInfo<'info>,
    /// CHECK: verified against the delegation program's own PDA derivation.
    #[account(mut)]
    pub delegation_record_pda: AccountInfo<'info>,
    /// CHECK: verified against the delegation program's own PDA derivation.
    #[account(mut)]
    pub delegation_metadata_pda: AccountInfo<'info>,
    /// CHECK: same rent payer passed to `request_force_undelegate`.
    #[account(mut)]
    pub delegation_rent_payer: AccountInfo<'info>,
    /// CHECK: only touched if the validator left a pending (uncommitted)
    /// commit behind; verified against the delegation program's own PDA
    /// derivation either way.
    #[account(mut)]
    pub commit_state_pda: AccountInfo<'info>,
    /// CHECK: see `commit_state_pda`.
    #[account(mut)]
    pub commit_record_pda: AccountInfo<'info>,
    /// CHECK: only read/written if a pending commit exists; a placeholder
    /// (e.g. `delegation_rent_payer` again) is fine otherwise — see
    /// `magicblock::delegation::force_undelegate_after_timeout`.
    #[account(mut)]
    pub commit_reimbursement: AccountInfo<'info>,
    /// CHECK: the delegation program itself.
    #[account(address = ephemeral_rollups_sdk::id())]
    pub delegation_program: AccountInfo<'info>,
}

pub fn handler_force_undelegate_after_timeout(
    ctx: Context<ForceUndelegateAfterTimeoutCtx>,
    game_id: u64,
) -> Result<()> {
    let game_id_bytes = game_id.to_le_bytes();
    let game_bump = ctx.bumps.game;
    crate::magicblock::delegation::force_undelegate_after_timeout(
        &ctx.accounts.game.to_account_info(),
        &ctx.accounts.owner_program.to_account_info(),
        &ctx.accounts.undelegation_request_pda.to_account_info(),
        &ctx.accounts.delegation_record_pda.to_account_info(),
        &ctx.accounts.delegation_metadata_pda.to_account_info(),
        &ctx.accounts.delegation_rent_payer.to_account_info(),
        &ctx.accounts.commit_state_pda.to_account_info(),
        &ctx.accounts.commit_record_pda.to_account_info(),
        &ctx.accounts.commit_reimbursement.to_account_info(),
        &game_id_bytes,
        game_bump,
    )
}
