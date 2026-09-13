use crate::constants::{CRANK_MAX_SECONDS_EARLY, CRANK_MAX_SLOT_DELAY};
use crate::errors::GameErrorCode;
use crate::lifecycle::clock;
use crate::state::Game;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CrankTimeCheckData {}

pub fn crank_time_check(ctx: Context<CrankTimeCheck>, _data: CrankTimeCheckData) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let clock = Clock::get()?;
    let now = clock.unix_timestamp;
    let slot = clock.slot;
    let game_id = game.game_id;

    let timeout_window = clock::timeout_window_seconds(game);
    let earliest_valid_timestamp = game
        .updated_at
        .checked_add(timeout_window)
        .ok_or(GameErrorCode::ArithmeticOverflow)?
        .checked_sub(CRANK_MAX_SECONDS_EARLY)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    require!(
        now >= earliest_valid_timestamp,
        GameErrorCode::CrankTooEarly
    );

    let expected_timeout_slot = (game.updated_at as u64)
        .saturating_add(timeout_window as u64)
        .saturating_div(400);
    let max_slot = expected_timeout_slot.saturating_add(CRANK_MAX_SLOT_DELAY);
    require!(slot <= max_slot, GameErrorCode::CrankTooLate);

    msg!(
        "crank_time_check: game {} at {} slot {}",
        game_id,
        now,
        slot
    );
    let timed_out = crate::lifecycle::terminal::finish_by_timeout_if_expired(game, now)?;
    if timed_out {
        msg!("crank_time_check: game {} flagged as timed out", game_id);
    }
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

    /// CHECK: Fully validated by the `constraint` below — its key must equal the
    /// `white` recorded on the (seed-verified) `game` PDA. The account itself is
    /// never read or written; the crank only needs both player keys present so
    /// the scheduled task's account list matches what was registered.
    #[account(constraint = white.key() == game.white @ crate::errors::GameErrorCode::InvalidPlayerAccount)]
    pub white: UncheckedAccount<'info>,

    /// CHECK: Same as `white` — key must equal the `black` recorded on `game`.
    #[account(constraint = black.key() == game.black @ crate::errors::GameErrorCode::InvalidPlayerAccount)]
    pub black: UncheckedAccount<'info>,
}
