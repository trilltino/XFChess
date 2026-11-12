use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct AcceptDraw<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    pub player: Signer<'info>,
}

pub fn handler(ctx: Context<AcceptDraw>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();
    let now = Clock::get()?.unix_timestamp;
    #[cfg(feature = "move-validation")]
    if game.to_account_info().data_len() != crate::fide::LEGACY_GAME_SPACE {
        let info = game.to_account_info();
        return crate::fide::finish_agreement(game, &info, player, now);
    }
    crate::lifecycle::terminal::finish_by_draw_agreement(game, player, now)
}
