//! Permissionless instruction that awards victory to the waiting player when an opponent
//! exceeds their move time limit. Anyone may call this once the timer has elapsed.

use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ClaimTimeout<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// Permissionless — any signer may trigger the timeout once elapsed.
    pub caller: Signer<'info>,
}

pub fn handler(ctx: Context<ClaimTimeout>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let now = Clock::get()?.unix_timestamp;
    crate::lifecycle::terminal::finish_by_timeout(game, now)
}
