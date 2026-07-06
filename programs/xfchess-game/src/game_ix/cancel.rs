//! Instruction to cancel a game and return escrowed wagers to both players.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct CancelGame<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// CHECK: Escrow PDA validated by seeds in constraint
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// The player initiating the cancel — must be white or black.
    #[account(mut)]
    pub player: Signer<'info>,
    /// CHECK: White player's wallet — must match game.white for accurate refund routing
    #[account(mut, constraint = white_authority.key() == game.white @ GameErrorCode::NotInGame)]
    pub white_authority: UncheckedAccount<'info>,
    /// CHECK: Black player's wallet — only validated when black has joined
    #[account(mut)]
    pub black_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CancelGame>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();

    let black_has_joined = game.black != Pubkey::default();
    crate::lifecycle::guards::require_undelegated(game)?;

    match game.status {
        GameStatus::WaitingForOpponent => {
            require!(player == game.white, GameErrorCode::NotGameCreator);
            game.status = GameStatus::Cancelled;
        }
        GameStatus::Active => {
            if game.move_count == 0 {
                require!(
                    player == game.white || player == game.black,
                    GameErrorCode::NotInGame
                );
                game.status = GameStatus::Cancelled;
            } else {
                let now = Clock::get()?.unix_timestamp;
                let inactivity_limit = 3600 * 24; // 24 hours
                require!(
                    now - game.updated_at > inactivity_limit,
                    GameErrorCode::GameNotExpired
                );
                game.status = GameStatus::Cancelled;
            }
        }
        _ => return Err(GameErrorCode::InvalidGameStatus.into()),
    }

    game.updated_at = Clock::get()?.unix_timestamp;

    // Refund Logic — both players get their wager back
    let wager_amount = game.wager_amount;
    if wager_amount > 0 {
        let game_id_bytes = _game_id.to_le_bytes();
        let bump = ctx.bumps.escrow_pda;
        let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[bump]]];

        // Always refund white (creator always escrowed)
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.escrow_pda.to_account_info(),
                    to: ctx.accounts.white_authority.to_account_info(),
                },
                escrow_seeds,
            ),
            wager_amount,
        )?;

        // Refund black only if they joined and escrowed their wager
        if black_has_joined {
            require!(
                ctx.accounts.black_authority.key() == game.black,
                GameErrorCode::NotInGame
            );
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.black_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                wager_amount,
            )?;
        }
    }

    Ok(())
}
