//! Instruction to create a new active wagered game context.

use crate::constants::{GAME_SEED, MAX_WAGER_AMOUNT, MIN_WAGER_LAMPORTS, WAGER_ESCROW_SEED};
use crate::errors::GameErrorCode;
use crate::game_ix::common::{init_game_fields, InitGameArgs};
use crate::state::{Game, MatchType};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64, wager_amount: u64, match_type: MatchType, platform_fee: u64, base_time_seconds: u64, increment_seconds: u16)]
pub struct CreateGame<'info> {
    #[account(
        init,
        payer = fee_payer,
        space = 8 + Game::INIT_SPACE,
        seeds = [GAME_SEED, &game_id.to_le_bytes()],
        bump
    )]
    pub game: Account<'info, Game>,
    /// CHECK: PDA for escrowing SOL.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(mut)]
    pub player: Signer<'info>,
    /// The VPS relayer wallet that covers rent and is reimbursed via fees_advanced.
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateGame>,
    game_id: u64,
    wager_amount: u64,
    match_type: MatchType,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<()> {
    require!(
        wager_amount <= MAX_WAGER_AMOUNT,
        GameErrorCode::WagerTooHigh
    );
    require!(
        wager_amount == 0 || wager_amount >= MIN_WAGER_LAMPORTS,
        GameErrorCode::StakeTooLow
    );

    init_game_fields(
        &mut ctx.accounts.game,
        InitGameArgs {
            game_id,
            white: ctx.accounts.player.key(),
            fee_payer: ctx.accounts.fee_payer.key(),
            wager_amount,
            match_type,
            platform_fee,
            base_time_seconds,
            increment_seconds,
            tournament_id: None,
        },
        Clock::get()?.unix_timestamp,
        ctx.bumps.game,
    )?;

    if wager_amount > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                System::id(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
            ),
            wager_amount,
        )?;
    }

    Ok(())
}
