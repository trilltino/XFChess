//! Instruction allowing a player to offer their opponent a draw.

use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;

/// Accounts for offering a draw in a game the signer is playing in.
#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct OfferDraw<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// The offering player — must be white or black.
    pub player: Signer<'info>,
}

/// Records the offer via `record_draw_offer`. Does not end the game — the
/// opponent must call `accept_draw` to finalize it.
pub fn handler(ctx: Context<OfferDraw>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();
    crate::lifecycle::terminal::record_draw_offer(game, player)
}
