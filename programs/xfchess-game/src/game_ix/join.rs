//! Instruction allowing a second player to match the wager and join a game.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct JoinGame<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [PROFILE_SEED, player.key().as_ref()], bump)]
    pub player_profile: Account<'info, PlayerProfile>,
    /// CHECK: PDA for escrowing SOL.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// CHECK: White player profile for cross-border fee calculation
    #[account(seeds = [PROFILE_SEED, game.white.as_ref()], bump)]
    pub white_profile: Account<'info, PlayerProfile>,
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<JoinGame>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();
    let fee_payer = ctx.accounts.fee_payer.key();

    require!(
        game.game_type == GameType::PvP,
        GameErrorCode::GameAlreadyFull
    ); // AI games are active by default

    // Platform fee was set at creation time from live SOL/GBP rate — no recalculation needed.
    let now = Clock::get()?.unix_timestamp;
    crate::lifecycle::transitions::join_waiting_game(game, player, fee_payer, now)?;

    if game.wager_amount > 0 && game.wager_token.is_none() {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                System::id(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
            ),
            game.wager_amount,
        )?;
    }

    Ok(())
}
