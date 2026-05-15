//! Instruction to create a new active wagered game context.

use crate::constants::{MAX_WAGER_AMOUNT, GAME_SEED, MOVE_LOG_SEED, WAGER_ESCROW_SEED, CREATE_GAME_COST};
use crate::state::{Game, GameStatus, GameResult, GameType, MatchType};
use crate::state::move_log::MoveLog;
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

/// Get country fee based on country code and match type.
/// Returns 0 for Free games, otherwise returns the country-specific fee.
fn get_country_fee(country: &str, match_type: MatchType) -> u64 {
    if match_type == MatchType::Free {
        return 0;
    }
    
    match country {
        "GB" => 0, // UK_FEE_LAMPORTS,
        "BR" => 0, // BRAZIL_FEE_LAMPORTS,
        "CA" => 0, // CANADA_FEE_LAMPORTS,
        "DE" => 0, // GERMANY_FEE_LAMPORTS,
        _ => 0, // Default to 0 for unsupported countries
    }
}

#[derive(Accounts)]
#[instruction(game_id: u64, wager_amount: u64, match_type: MatchType, country: String, base_time_seconds: u64, increment_seconds: u16)]
pub struct CreateGame<'info> {
    #[account(
        init, 
        payer = fee_payer, 
        space = 8 + Game::INIT_SPACE, 
        seeds = [GAME_SEED, &game_id.to_le_bytes()], 
        bump
    )]
    pub game: Account<'info, Game>,
    #[account(
        init, 
        payer = fee_payer, 
        space = MoveLog::LEN, 
        seeds = [MOVE_LOG_SEED, &game_id.to_le_bytes()], 
        bump
    )]
    pub move_log: Account<'info, MoveLog>,
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
    country: String,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    game.game_id = game_id;
    game.white = ctx.accounts.player.key();
    game.black = Pubkey::default();
    game.status = GameStatus::WaitingForOpponent;
    game.result = GameResult::None;
    game.fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string();
    game.move_count = 0;
    game.turn = 1;
    game.created_at = Clock::get()?.unix_timestamp;
    game.updated_at = game.created_at;
    game.wager_amount = wager_amount;
    game.wager_token = None;
    game.game_type = GameType::PvP;
    game.match_type = match_type.clone();
    game.country_fee = get_country_fee(&country, match_type.clone());
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

    let move_log = &mut ctx.accounts.move_log;
    move_log.game_id = game_id;
    move_log.moves = Vec::new();
    move_log.timestamps = Vec::new();
    move_log.player_signatures = Vec::new();
    move_log.nonce = 0;

    msg!(
        "Game {} created. Wager: {} lamports",
        game_id,
        wager_amount
    );
    Ok(())
}
