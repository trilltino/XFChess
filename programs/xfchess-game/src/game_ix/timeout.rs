//! Permissionless instruction that awards victory to the waiting player when an opponent
//! exceeds their move time limit. Anyone may call this once the timer has elapsed.

use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;

/// Accounts for claiming a timeout victory. `caller` need not be a player in
/// the game — anyone may crank this once a clock has expired.
#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ClaimTimeout<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// Permissionless — any signer may trigger the timeout once elapsed.
    pub caller: Signer<'info>,
}

/// Awards victory to whichever player did not exceed their clock, via
/// `finish_by_timeout`. No-op error if no clock has actually expired.
pub fn handler(ctx: Context<ClaimTimeout>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let now = Clock::get()?.unix_timestamp;
    crate::lifecycle::terminal::finish_by_timeout(game, now)
}
