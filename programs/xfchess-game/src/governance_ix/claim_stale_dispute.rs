//! Permissionless auto-resolution of a dispute left unreviewed past its TTL.
//!
//! After `DISPUTE_TTL_SECS` (7 days) any signer may split the pot 50/50 so funds
//! are never locked by platform inaction. Each player gets their full stake back
//! (`pot / 2 == wager`), the challenger's bond is refunded (no fault was ruled),
//! and the game is set to `Settled` so `finalize_game` cannot re-process it.

use crate::common::escrow;
use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ClaimStaleDispute<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [b"dispute", &game_id.to_le_bytes()], bump)]
    pub dispute_record: Account<'info, DisputeRecord>,
    /// System-owned wager escrow PDA — paid out via signed CPI.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: SystemAccount<'info>,
    /// Permissionless — any signer may trigger once the TTL has elapsed.
    pub caller: Signer<'info>,
    /// White player wallet — must match game.white.
    #[account(mut, constraint = white_authority.key() == game.white @ GameErrorCode::NotInGame)]
    pub white_authority: SystemAccount<'info>,
    /// Black player wallet — must match game.black.
    #[account(mut, constraint = black_authority.key() == game.black @ GameErrorCode::NotInGame)]
    pub black_authority: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimStaleDispute>, game_id: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.dispute_record.status == DisputeStatus::Pending,
        GameErrorCode::GameNotDisputed
    );
    require!(
        now > ctx.accounts.dispute_record.expires_at,
        GameErrorCode::TimeoutNotExpired
    );

    let challenger = ctx.accounts.dispute_record.challenger;
    let bond = ctx.accounts.dispute_record.bond_amount;
    let game_white = ctx.accounts.game.white;
    let wager = ctx.accounts.game.wager_amount;
    let escrow_bump = ctx.bumps.escrow_pda;

    {
        let dispute = &mut ctx.accounts.dispute_record;
        dispute.status = DisputeStatus::Dismissed;
        dispute.resolution = "Auto-resolved: dispute TTL expired — escrow split 50/50".to_string();
        dispute.resolved_at = Some(now);
    }
    {
        let game = &mut ctx.accounts.game;
        game.result = GameResult::Draw;
        game.status = GameStatus::Settled;
        game.updated_at = now;
    }

    // Split the full pot: each player gets their stake back (pot / 2 == wager).
    if wager > 0 {
        let pot = escrow::pot(wager)?;
        if ctx.accounts.escrow_pda.lamports() >= pot {
            let each = pot / 2;
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
        }
    }

    // Refund the challenger's bond (no fault was ruled).
    if bond > 0 {
        let dispute_info = ctx.accounts.dispute_record.to_account_info();
        let dest = if challenger == game_white {
            ctx.accounts.white_authority.as_ref()
        } else {
            ctx.accounts.black_authority.as_ref()
        };
        escrow::debit_program_pda(&dispute_info, dest, bond)?;
    }

    Ok(())
}
