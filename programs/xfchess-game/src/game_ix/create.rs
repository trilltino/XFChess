//! Instruction to create a new active wagered game context.

use crate::constants::{MAX_WAGER_AMOUNT, GAME_SEED, WAGER_ESCROW_SEED, CREATE_GAME_COST};
use crate::state::{Game, GameStatus, GameResult, GameType, MatchType};
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64, wager_amount: u64, match_type: MatchType, platform_fee: u64, base_time_seconds: u64, increment_seconds: u16)]
pub struct CreateGame<'info> {
    #[account(
        init, 
        payer = fee_payer, 
        space = 8 + Game::INIT_SPACE, 
        seeds = [GAME_SEED, &game_id.to_le_bytes()], 
        bump
    )]
    pub game: Account<'info, Game>,
    /// CHECK: PDA for escrowing SOL.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(mut)]
    pub player: Signer<'info>,
    /// The VPS relayer wallet that covers rent and is reimbursed via fees_advanced.
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateGame>,
    game_id: u64,
    wager_amount: u64,
    match_type: MatchType,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    game.game_id = game_id;
    game.white = ctx.accounts.player.key();
    game.black = Pubkey::default();
    game.status = GameStatus::WaitingForOpponent;
    game.result = GameResult::None;
    // Starting position (from chess_logic_on_chain::shakmaty or equivalent starting board bytes)
    #[cfg(feature = "move-validation")]
    {
        game.board_state = chess_logic_on_chain::nimzovich_engine::CompactBoard::starting_position().to_bytes();
    }
    #[cfg(not(feature = "move-validation"))]
    {
        game.board_state = [0; 68]; // default zeroed if validation is off to save compute
    }
    game.move_count = 0;
    game.turn = 1;
    game.nonce = 0; // Initialize nonce to zero
    game.created_at = Clock::get()?.unix_timestamp;
    game.updated_at = game.created_at;
    game.wager_amount = wager_amount;
    game.wager_token = None;
    game.game_type = GameType::PvP;
    game.match_type = match_type.clone();
    game.country_fee = if match_type == MatchType::Free { 0 } else { platform_fee };
    game.base_time_seconds = base_time_seconds;
    game.increment_seconds = increment_seconds;
    game.bump = ctx.bumps.game;
    game.fee_payer = ctx.accounts.fee_payer.key();
    game.fees_advanced = CREATE_GAME_COST;
    game.is_delegated = false;

    require!(wager_amount <= MAX_WAGER_AMOUNT, GameErrorCode::WagerTooHigh);

    if wager_amount > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
            ),
            wager_amount,
        )?;
    }


    Ok(())
}
