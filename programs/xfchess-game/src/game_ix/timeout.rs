use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ClaimTimeout<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    pub caller: Signer<'info>,
}

#[event]
pub struct TimeoutClaimed {
    pub game_id: u64,
    pub caller: Pubkey,
    pub status: u8,
    pub winner: Pubkey,
    pub timestamp: i64,
}

pub fn handler(ctx: Context<ClaimTimeout>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let now = Clock::get()?.unix_timestamp;
    crate::lifecycle::terminal::finish_by_timeout(game, now)?;
    let winner = match game.result {
        GameResult::Winner(winner) => winner,
        _ => Pubkey::default(),
    };
    emit!(TimeoutClaimed {
        game_id: _game_id,
        caller: ctx.accounts.caller.key(),
        status: game.status as u8,
        winner,
        timestamp: now,
    });
    Ok(())
}
