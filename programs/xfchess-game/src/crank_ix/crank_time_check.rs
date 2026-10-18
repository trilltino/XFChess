use crate::constants::CRANK_MAX_SECONDS_EARLY;
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

    // A late run still resolves an expired game, so there is no upper bound:
    // slot height and wall-clock time are unrelated on the ER.
    let earliest_valid_timestamp = game
        .updated_at
        .saturating_add(clock::timeout_window_seconds(game))
        .saturating_sub(CRANK_MAX_SECONDS_EARLY);
    require!(
        now >= earliest_valid_timestamp,
        GameErrorCode::CrankTooEarly
    );

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

    /// CHECK: Must match game.white on the seed-verified Game PDA.
    /// Included only to match the registered crank account list.
    #[account(constraint = white.key() == game.white @ crate::errors::GameErrorCode::InvalidPlayerAccount)]
    pub white: UncheckedAccount<'info>,

    /// CHECK: Same as `white` — key must equal the `black` recorded on `game`.
    #[account(constraint = black.key() == game.black @ crate::errors::GameErrorCode::InvalidPlayerAccount)]
    pub black: UncheckedAccount<'info>,
}
