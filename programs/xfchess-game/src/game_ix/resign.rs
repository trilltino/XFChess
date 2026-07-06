//! Instruction allowing a player to concede defeat.

use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ResignGame<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// The resigning player — must be white or black.
    pub player: Signer<'info>,
}

pub fn handler(ctx: Context<ResignGame>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();
    let now = Clock::get()?.unix_timestamp;
    crate::lifecycle::terminal::finish_by_resign(game, player, now)
}
