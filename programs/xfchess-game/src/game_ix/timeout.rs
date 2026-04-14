//! Permissionless instruction that awards victory to the waiting player when an opponent
//! exceeds their move time limit. Anyone may call this once the timer has elapsed.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ClaimTimeout<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// CHECK: Escrow PDA validated by seeds
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// Permissionless — any signer may trigger the timeout once elapsed.
    #[account(mut)]
    pub caller: Signer<'info>,
    /// CHECK: White player's wallet — must match game.white
    #[account(mut, constraint = white_authority.key() == game.white @ GameErrorCode::NotInGame)]
    pub white_authority: UncheckedAccount<'info>,
    /// CHECK: Black player's wallet — must match game.black
    #[account(mut, constraint = black_authority.key() == game.black @ GameErrorCode::NotInGame)]
    pub black_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimTimeout>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let clock = Clock::get()?;

    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );
    require!(
        game.time_per_move > 0,
        GameErrorCode::NoTimeLimit
    );
    require!(
        clock.unix_timestamp - game.updated_at > game.time_per_move as i64,
        GameErrorCode::TimeoutNotExpired
    );

    // Whose turn timed out?
    // turn starts at 1 (white) and increments each half-move.
    // Odd turn = white to move; even turn = black to move.
    let white_timed_out = game.turn % 2 == 1;
    let winner = if white_timed_out {
        game.black
    } else {
        game.white
    };

    game.result = GameResult::Winner(winner);
    game.status = GameStatus::Finished;
    game.updated_at = clock.unix_timestamp;

    msg!(
        "XFChess: Timeout — {} wins (opponent exceeded {}s time limit)",
        winner,
        game.time_per_move
    );

    // Pay out escrow immediately
    let wager_amount = game.wager_amount;
    if wager_amount > 0 && game.wager_token.is_none() {
        let pot = wager_amount * 2;
        let game_id_bytes = _game_id.to_le_bytes();
        let bump = ctx.bumps.escrow_pda;
        let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[bump]]];

        let dest = if winner == game.white {
            ctx.accounts.white_authority.to_account_info()
        } else {
            ctx.accounts.black_authority.to_account_info()
        };

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.escrow_pda.to_account_info(),
                    to: dest,
                },
                escrow_seeds,
            ),
            pot,
        )?;
    }

    Ok(())
}
