//! Permissionless instruction that auto-resolves an expired, unreviewed dispute.
//!
//! If a DisputeRecord remains Pending for more than DISPUTE_TTL_SECS (7 days),
//! any signer may call this instruction to split the escrow 50/50 between both
//! players. This prevents funds being locked indefinitely due to platform failure.

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
    /// CHECK: Escrow PDA validated by seeds
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// Permissionless — any signer may trigger once the TTL has elapsed.
    pub caller: Signer<'info>,
    /// CHECK: White player wallet — must match game.white
    #[account(mut, constraint = white_authority.key() == game.white @ GameErrorCode::NotInGame)]
    pub white_authority: UncheckedAccount<'info>,
    /// CHECK: Black player wallet — must match game.black
    #[account(mut, constraint = black_authority.key() == game.black @ GameErrorCode::NotInGame)]
    pub black_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimStaleDispute>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let dispute = &mut ctx.accounts.dispute_record;
    let clock = Clock::get()?;

    require!(
        dispute.status == DisputeStatus::Pending,
        GameErrorCode::GameNotDisputed
    );
    require!(
        clock.unix_timestamp > dispute.expires_at,
        GameErrorCode::TimeoutNotExpired
    );

    dispute.status = DisputeStatus::Dismissed;
    dispute.resolution = "Auto-resolved: dispute TTL expired — escrow split 50/50".to_string();
    dispute.resolved_at = Some(clock.unix_timestamp);

    game.result = GameResult::Draw;
    game.status = GameStatus::Finished;
    game.updated_at = clock.unix_timestamp;

    let wager_amount = game.wager_amount;
    if wager_amount > 0 {
        let half = wager_amount / 2;
        let rent = Rent::get()?;
        let white_balance_after = ctx.accounts.white_authority.lamports().saturating_add(half);
        let black_balance_after = ctx.accounts.black_authority.lamports().saturating_add(half);
        let game_id_bytes = _game_id.to_le_bytes();
        let bump = ctx.bumps.escrow_pda;
        let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[bump]]];

        if rent.is_exempt(white_balance_after, ctx.accounts.white_authority.data_len()) {
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.white_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                half,
            )?;
        } else {
            msg!("Skipping refund to white player: not rent-exempt after transfer");
        }
        if rent.is_exempt(black_balance_after, ctx.accounts.black_authority.data_len()) {
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.black_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                half,
            )?;
        } else {
            msg!("Skipping refund to black player: not rent-exempt after transfer");
        }
    }

    msg!(
        "XFChess: Stale dispute on game {} auto-resolved — escrow split 50/50",
        _game_id
    );

    Ok(())
}
