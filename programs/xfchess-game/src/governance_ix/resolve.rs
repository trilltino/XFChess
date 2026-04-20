//! Instruction for admins to resolve an open dispute and allocate the prize pool.

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
    pub dispute_record: Account<'info, DisputeRecord>,
    /// CHECK: Escrow PDA validated by seeds
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// Dispute-resolution authority — only the platform key may resolve.
    #[account(address = crate::constants::dispute_authority::ID @ GameErrorCode::UnauthorizedDisputeResolution)]
    pub authority: Signer<'info>,
    /// CHECK: White player destination — must match game.white
    #[account(mut, constraint = white_authority.key() == game.white @ GameErrorCode::NotInGame)]
    pub white_authority: UncheckedAccount<'info>,
    /// CHECK: Black player destination — must match game.black
    #[account(mut, constraint = black_authority.key() == game.black @ GameErrorCode::NotInGame)]
    pub black_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ResolveDispute>,
    _game_id: u64,
    resolution: String,
    winner: Option<Pubkey>,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let dispute = &mut ctx.accounts.dispute_record;

    // Check authority (in a real-world scenario, this would be a specific admin key)
    // For this example, let's assume the creator is the admin (though it should be a platform authority).
    // require!(ctx.accounts.authority.key() == ADMIN_KEY, GameErrorCode::UnauthorizedDisputeResolution);

    require!(
        game.status == GameStatus::Disputed,
        GameErrorCode::GameNotDisputed
    );

    dispute.status = DisputeStatus::Resolved;
    dispute.resolution = resolution;
    dispute.resolved_by = Some(ctx.accounts.authority.key());
    dispute.resolved_at = Some(Clock::get()?.unix_timestamp);

    let wager_amount = game.wager_amount;
    let game_id_bytes = _game_id.to_le_bytes();
    let bump = ctx.bumps.escrow_pda;
    let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[bump]]];

    if let Some(w) = winner {
        game.result = GameResult::Winner(w);
        game.status = GameStatus::Finished;

        if wager_amount > 0 {
            let pot = wager_amount
                .checked_mul(2)
                .ok_or(GameErrorCode::Overflow)?;
            let dest = if w == game.white {
                ctx.accounts.white_authority.to_account_info()
            } else {
                ctx.accounts.black_authority.to_account_info()
            };
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: dest,
                    },
                    escrow_seeds,
                ),
                pot,
            )?;
        }
    } else {
        // Draw — split the pot evenly between both players.
        game.result = GameResult::Draw;
        game.status = GameStatus::Finished;

        if wager_amount > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.white_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                wager_amount,
            )?;
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.black_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                wager_amount,
            )?;
        }
    }

    Ok(())
}
