//! Automatic time check crank instruction.

use crate::state::Game;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CrankTimeCheckData {}

pub fn crank_time_check(ctx: Context<CrankTimeCheck>, _data: CrankTimeCheckData) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let now = Clock::get()?.unix_timestamp;
    crate::lifecycle::terminal::finish_by_timeout_if_expired(game, now)?;
    Ok(())
}

#[derive(Accounts)]
pub struct CrankTimeCheck<'info> {
    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, Game>,

    /// CHECK: White player reference for scheduled-task account metas.
    pub white: AccountInfo<'info>,

    /// CHECK: Black player reference for scheduled-task account metas.
    pub black: AccountInfo<'info>,
}
