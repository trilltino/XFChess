use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct CancelGame<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: SystemAccount<'info>,
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(mut, constraint = white_authority.key() == game.white @ GameErrorCode::NotInGame)]
    pub white_authority: SystemAccount<'info>,
    #[account(mut, constraint = black_authority.key() == game.black @ GameErrorCode::NotInGame)]
    pub black_authority: SystemAccount<'info>,
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

    let wager_amount = game.wager_amount;
    if wager_amount > 0 {
        let game_id_bytes = _game_id.to_le_bytes();
        let bump = ctx.bumps.escrow_pda;
        let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[bump]]];

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                System::id(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.escrow_pda.to_account_info(),
                    to: ctx.accounts.white_authority.to_account_info(),
                },
                escrow_seeds,
            ),
            wager_amount,
        )?;

        if black_has_joined {
            require!(
                ctx.accounts.black_authority.key() == game.black,
                GameErrorCode::NotInGame
            );
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    System::id(),
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
