//! Instruction allowing a player to accept their opponent's pending draw offer.

use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;

/// Accounts for accepting a pending draw offer.
#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct AcceptDraw<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// The accepting player — must be white or black, and must not be the
    /// player who placed the offer being accepted.
    pub player: Signer<'info>,
}

/// Ends the game as a draw via `finish_by_draw_agreement`. Settlement (pot
/// split, ELO, stats) happens later through the existing `finalize_game`
/// path, same as every other terminal result.
pub fn handler(ctx: Context<AcceptDraw>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();
    let now = Clock::get()?.unix_timestamp;
    crate::lifecycle::terminal::finish_by_draw_agreement(game, player, now)
}
