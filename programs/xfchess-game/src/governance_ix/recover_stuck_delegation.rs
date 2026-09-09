use crate::common::escrow;
use crate::constants::*;
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RecoverStuckDelegation<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: AccountInfo<'info>,
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: SystemAccount<'info>,
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: SystemAccount<'info>,
    #[account(mut)]
    pub white_authority: SystemAccount<'info>,
    #[account(mut)]
    pub black_authority: SystemAccount<'info>,
    #[account(constraint = dispute_authority.key() == crate::constants::dispute_authority::ID @ GameErrorCode::UnauthorizedDisputeResolution)]
    pub dispute_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

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
    let mut share_paid = 0u64;
    if escrow_lamports > 0 {
        let each = escrow_lamports / 2;
        share_paid = each;
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

    emit!(crate::events::StuckDelegationRecovered {
        game_id,
        dispute_authority: ctx.accounts.dispute_authority.key(),
        white_authority: ctx.accounts.white_authority.key(),
        black_authority: ctx.accounts.black_authority.key(),
        white_share: share_paid,
        black_share: share_paid,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
