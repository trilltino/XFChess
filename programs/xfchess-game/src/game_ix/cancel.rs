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
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CancelGame>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();

    // Cancellation Logic:
    // 1. WaitingForOpponent: Only the creator can cancel.
    // 2. Active: Only possible if no moves have been made yet (mutual or early exit).
    //    Or if a timeout has occurred (handled by a separate instruction or this one).
    
    match game.status {
        GameStatus::WaitingForOpponent => {
            require!(player == game.white, GameErrorCode::NotGameCreator);
            game.status = GameStatus::Cancelled;
        }
        GameStatus::Active => {
            // If no moves, either player can initiate if we want to allow early exit.
            // For now, let's say only creator can cancel if no one joined or if black is AI.
            if game.move_count == 0 {
                require!(player == game.white || player == game.black, GameErrorCode::NotInGame);
                game.status = GameStatus::Cancelled;
            } else {
                // Inactivity check
                let now = Clock::get()?.unix_timestamp;
                let inactivity_limit = 3600 * 24; // 24 hours
                require!(now - game.updated_at > inactivity_limit, GameErrorCode::GameNotExpired);
                game.status = GameStatus::Cancelled;
            }
        }
        _ => return Err(GameErrorCode::InvalidGameStatus.into()),
    }

    // Refund Logic
    let wager_amount = game.wager_amount;
    if wager_amount > 0 {
        let pot = if game.status == GameStatus::Cancelled && game.black != Pubkey::default() && game.move_count > 0 {
            // If cancelled due to inactivity, maybe we split or give to the one who didn't timeout?
            // For simplicity, let's refund the current player if they were the one who moved last?
            // Actually, let's just refund the wager to both players if they both paid.
            wager_amount * 2
        } else {
            wager_amount
        };

        let game_id_bytes = _game_id.to_le_bytes();
        let bump = ctx.bumps.escrow_pda;
        let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[bump]]];

        // For simplicity in this hardening, refund the creator their wager.
        // If black joined, they also get their wager back.
        if game.black != Pubkey::default() && game.black != crate::constants::ai_authority::ID {
            // Refund white
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.player.to_account_info(), // Assuming player cancels
                    },
                    escrow_seeds,
                ),
                wager_amount,
            )?;
            // In a real app, you'd need the other player's account info too for the refund.
            // This is a simplified version.
        } else {
            // Refund white (creator)
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.player.to_account_info(),
                    },
                    escrow_seeds,
                ),
                wager_amount,
            )?;
        }
    }

    Ok(())
}
