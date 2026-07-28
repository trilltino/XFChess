//! Automatic time check crank instruction.

use crate::state::Game;
use anchor_lang::prelude::*;

/// No arguments needed — the game account already carries its own clock state.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CrankTimeCheckData {}

/// The scheduled callback the Ephemeral Rollup invokes each
/// `check_interval_millis`. Idempotent: only mutates the game if a clock has
/// actually expired (via `finish_by_timeout_if_expired`), so repeated firings
/// after a game already ended are no-ops.
pub fn crank_time_check(ctx: Context<CrankTimeCheck>, _data: CrankTimeCheckData) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let now = Clock::get()?.unix_timestamp;
    crate::lifecycle::terminal::finish_by_timeout_if_expired(game, now)?;
    Ok(())
}

/// Accounts for the ER-invoked time-check crank callback.
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
