//! Recovers wager escrow from a `Game` PDA that was force-undelegated after
//! its ER validator went dark (see `delegation_ix::force_recovery`).
//!
//! `force_undelegate_after_timeout` hands the `Game` PDA back wiped to zero
//! bytes — no players, wager amount, move history, or result survive it, by
//! the delegation program's own design (it refuses to trust a validator's
//! unfinalized commit, and doesn't preserve the account's prior data
//! either). That means the normal `finalize_game` path can never run
//! against it: there is no on-chain record left of who was playing or how
//! much was staked.
//!
//! This instruction is the escape hatch for that specific dead end. It
//! trusts the same `dispute_authority` key that already single-handedly
//! rules on `resolve_dispute` — attesting `white_authority`/`black_authority`
//! from off-chain records (this game's own `create_game`/`join_game`
//! transaction history, which is immutable ledger history even though the
//! live `Game` account is gone) — and, since there's no trustworthy way to
//! recover who was winning, splits whatever is actually sitting in the
//! escrow 50/50, mirroring `claim_stale_dispute`'s "no fault ruled" refund.
//! It never touches ELO or profiles: a game that never reached a verifiable
//! on-chain result shouldn't move anyone's rating.

use crate::common::escrow;
use crate::constants::*;
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

/// Accounts for `recover_stuck_delegation`.
#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RecoverStuckDelegation<'info> {
    /// CHECK: ownership/emptiness checked in the handler — see module docs
    /// for why this can't be a typed `Account<'info, Game>`.
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: AccountInfo<'info>,
    /// System-owned per-game wager escrow PDA (funds moved out via signed CPI).
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: SystemAccount<'info>,
    /// Platform treasury vault — receives the wiped `Game` PDA's reclaimed rent.
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: SystemAccount<'info>,
    /// White player wallet, attested by `dispute_authority` from off-chain
    /// records — cannot be verified on-chain since `game` no longer holds it.
    #[account(mut)]
    pub white_authority: SystemAccount<'info>,
    /// Black player wallet — see `white_authority`.
    #[account(mut)]
    pub black_authority: SystemAccount<'info>,
    /// The same authority trusted to single-handedly resolve disputes.
    #[account(constraint = dispute_authority.key() == crate::constants::dispute_authority::ID @ GameErrorCode::UnauthorizedDisputeResolution)]
    pub dispute_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Verifies `game` is actually in the post-force-undelegate wiped state
/// (owned by this program, zero data) before moving any funds — the
/// signature that this game genuinely went through the ER-unavailability
/// escape hatch, not a live game this instruction has no business touching.
pub fn handler(ctx: Context<RecoverStuckDelegation>, game_id: u64) -> Result<()> {
    require_keys_eq!(
        *ctx.accounts.game.owner,
        crate::ID,
        GameErrorCode::GameNotStuckDelegation
    );
    require!(
        ctx.accounts.game.data_is_empty(),
        GameErrorCode::GameNotStuckDelegation
    );

    let escrow_lamports = ctx.accounts.escrow_pda.lamports();
    if escrow_lamports > 0 {
        let each = escrow_lamports / 2;
        let escrow_bump = ctx.bumps.escrow_pda;
        let sp = &ctx.accounts.system_program;
        let escrow = &ctx.accounts.escrow_pda;

        escrow::require_rent_exempt_after(ctx.accounts.white_authority.as_ref(), each)?;
        escrow::require_rent_exempt_after(ctx.accounts.black_authority.as_ref(), each)?;

        escrow::pay_from_game_escrow(
            sp,
            escrow,
            ctx.accounts.white_authority.as_ref(),
            each,
            game_id,
            escrow_bump,
        )?;
        escrow::pay_from_game_escrow(
            sp,
            escrow,
            ctx.accounts.black_authority.as_ref(),
            each,
            game_id,
            escrow_bump,
        )?;
        // Any single-lamport odd remainder is left in escrow_pda — harmless.
    }

    // Reclaim the wiped Game PDA's own (near-zero, zero-data rent-exempt
    // minimum) lamports to the treasury rather than leaving a dust PDA
    // sitting around forever. `game` is program-owned, so a direct debit
    // (not a signed system-program transfer) is the correct move here.
    let game_lamports = ctx.accounts.game.lamports();
    if game_lamports > 0 {
        **ctx.accounts.game.try_borrow_mut_lamports()? -= game_lamports;
        **ctx
            .accounts
            .treasury_vault
            .to_account_info()
            .try_borrow_mut_lamports()? += game_lamports;
    }

    Ok(())
}
