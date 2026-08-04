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
    let game_id = game.game_id;
    // This is the only instruction in the delegate/undelegate/crank family
    // that's invoked autonomously by MagicBlock's scheduler rather than by
    // the backend (which already logs every other step it calls around).
    // Without this, there's no way to tell from `solana logs`/Solscan
    // whether a given interval fired, or what it decided.
    msg!("crank_time_check: game {} at {}", game_id, now);
    let timed_out = crate::lifecycle::terminal::finish_by_timeout_if_expired(game, now)?;
    if timed_out {
        msg!("crank_time_check: game {} flagged as timed out", game_id);
    }
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

    /// CHECK: White player reference for scheduled-task account metas — not
    /// read by this instruction's body, but constrained to `game.white`
    /// anyway; see the same constraint in `schedule_time_check::ScheduleTimeCheck`.
    #[account(constraint = white.key() == game.white @ crate::errors::GameErrorCode::InvalidPlayerAccount)]
    pub white: AccountInfo<'info>,

    /// CHECK: Black player reference for scheduled-task account metas — see `white` above.
    #[account(constraint = black.key() == game.black @ crate::errors::GameErrorCode::InvalidPlayerAccount)]
    pub black: AccountInfo<'info>,
}
