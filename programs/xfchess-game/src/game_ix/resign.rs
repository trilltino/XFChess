//! Instruction allowing a player to concede defeat, awarding the opponent the wager.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ResignGame<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// CHECK: Escrow PDA validated by seeds
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// The resigning player — must be white or black.
    #[account(mut)]
    pub player: Signer<'info>,
    /// CHECK: White player wallet — must match game.white
    #[account(mut, constraint = white_authority.key() == game.white @ GameErrorCode::NotInGame)]
    pub white_authority: UncheckedAccount<'info>,
    /// CHECK: Black player wallet — must match game.black
    #[account(mut, constraint = black_authority.key() == game.black @ GameErrorCode::NotInGame)]
    pub black_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ResignGame>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();

    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );
    require!(
        player == game.white || player == game.black,
        GameErrorCode::NotInGame
    );

    // The opponent of the resigning player is the winner
    let winner = if player == game.white {
        game.black
    } else {
        game.white
    };

    game.result = GameResult::Winner(winner);
    game.status = GameStatus::Finished;
    game.updated_at = Clock::get()?.unix_timestamp;


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
