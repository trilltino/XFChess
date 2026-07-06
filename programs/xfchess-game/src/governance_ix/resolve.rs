//! Dispute resolution by the platform dispute authority.
//!
//! Allocates the pot per the authority's ruling and settles the bond the
//! challenger posted in `dispute_game`: refunded if the challenger's claim is
//! upheld (or the game is ruled a draw), forfeited to the treasury if the ruling
//! goes to the opponent. The game is set to `Settled` so `finalize_game` cannot
//! re-process it.

use crate::common::escrow;
use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ResolveDispute<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [b"dispute", &game_id.to_le_bytes()], bump)]
    pub dispute: Account<'info, DisputeRecord>,
    /// System-owned wager escrow PDA — paid out via signed CPI.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: SystemAccount<'info>,
    /// Only the platform dispute authority may resolve.
    #[account(address = crate::constants::dispute_authority::ID @ GameErrorCode::UnauthorizedDisputeResolution)]
    pub dispute_authority: Signer<'info>,
    /// White player wallet — must match game.white.
    #[account(mut, constraint = white_account.key() == game.white @ GameErrorCode::NotInGame)]
    pub white_account: SystemAccount<'info>,
    /// Black player wallet — must match game.black.
    #[account(mut, constraint = black_account.key() == game.black @ GameErrorCode::NotInGame)]
    pub black_account: SystemAccount<'info>,
    /// Platform treasury — seeded PDA so funds can't be redirected.
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub platform_treasury: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ResolveDispute>,
    game_id: u64,
    resolution: String,
    winner: Option<Pubkey>,
) -> Result<()> {
    crate::governance_ix::resolution::require_text_fits(&resolution)?;
    require!(
        ctx.accounts.dispute.status == DisputeStatus::Pending,
        GameErrorCode::GameNotDisputed
    );
    require!(
        ctx.accounts.dispute.game_id == game_id,
        GameErrorCode::InvalidGameStatus
    );

    let game_white = ctx.accounts.game.white;
    let result = crate::governance_ix::resolution::validate_resolution(&ctx.accounts.game, winner)?;

    let now = Clock::get()?.unix_timestamp;
    let challenger = ctx.accounts.dispute.challenger;
    let bond = ctx.accounts.dispute.bond_amount;
    let wager = ctx.accounts.game.wager_amount;
    let escrow_bump = ctx.bumps.escrow_pda;

    crate::governance_ix::resolution::apply_resolution(
        &mut ctx.accounts.game,
        &mut ctx.accounts.dispute,
        result,
        resolution,
        ctx.accounts.dispute_authority.key(),
        now,
    )?;

    // ── Pot payout (signed CPI from the system-owned escrow) ──────────────────
    if wager > 0 {
        let pot = escrow::pot(wager)?;
        if ctx.accounts.escrow_pda.lamports() >= pot {
            let sp = &ctx.accounts.system_program;
            let escrow = &ctx.accounts.escrow_pda;
            let fee = DISPUTE_RESOLUTION_COST_LAMPORTS.min(pot);
            let distributable = pot.saturating_sub(fee);

            escrow::pay_from_game_escrow(
                sp,
                escrow,
                ctx.accounts.platform_treasury.as_ref(),
                fee,
                game_id,
                escrow_bump,
            )?;
            match winner {
                Some(k) => {
                    let dest = if k == game_white {
                        ctx.accounts.white_account.as_ref()
                    } else {
                        ctx.accounts.black_account.as_ref()
                    };
                    escrow::pay_from_game_escrow(
                        sp,
                        escrow,
                        dest,
                        distributable,
                        game_id,
                        escrow_bump,
                    )?;
                }
                None => {
                    let each = distributable / 2;
                    escrow::pay_from_game_escrow(
                        sp,
                        escrow,
                        ctx.accounts.white_account.as_ref(),
                        each,
                        game_id,
                        escrow_bump,
                    )?;
                    escrow::pay_from_game_escrow(
                        sp,
                        escrow,
                        ctx.accounts.black_account.as_ref(),
                        each,
                        game_id,
                        escrow_bump,
                    )?;
                }
            }
        }
    }

    // ── Dispute bond (held in the program-owned dispute PDA) ──────────────────
    // Refund when the challenger's claim is upheld or the game is ruled a draw;
    // forfeit to the treasury when the ruling goes to the opponent.
    if bond > 0 {
        let refund = match winner {
            Some(k) => k == challenger,
            None => true,
        };
        let dispute_info = ctx.accounts.dispute.to_account_info();
        let dest = if refund {
            if challenger == game_white {
                ctx.accounts.white_account.as_ref()
            } else {
                ctx.accounts.black_account.as_ref()
            }
        } else {
            ctx.accounts.platform_treasury.as_ref()
        };
        escrow::debit_program_pda(&dispute_info, dest, bond)?;
    }

    Ok(())
}
