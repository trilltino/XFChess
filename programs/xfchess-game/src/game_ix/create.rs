//! Instruction to create a new active wagered game context.

use crate::constants::*;
use crate::state::*;
use crate::errors::*;
use anchor_lang::prelude::*;

/// Get country fee based on country code and match type.
/// Returns 0 for Free games, otherwise returns the country-specific fee.
fn get_country_fee(country: &str, match_type: MatchType) -> u64 {
    if match_type == MatchType::Free {
        return 0;
    }
    
    match country {
        "GB" => UK_FEE_LAMPORTS,
        "BR" => BRAZIL_FEE_LAMPORTS,
        "CA" => CANADA_FEE_LAMPORTS,
        "DE" => GERMANY_FEE_LAMPORTS,
        _ => 0, // Default to 0 for unsupported countries
    }
}

#[derive(Accounts)]
#[instruction(game_id: u64, wager_amount: u64, game_type: GameType, match_type: MatchType, country: String, time_per_move: u16)]
pub struct CreateGame<'info> {
    #[account(
        init, 
        payer = player, 
        space = 8 + Game::INIT_SPACE, 
        seeds = [GAME_SEED, &game_id.to_le_bytes()], 
        bump
    )]
    pub game: Account<'info, Game>,
    #[account(
        init, 
        payer = player, 
        space = 10240, // Sufficient space for moves, timestamps, and signatures
        seeds = [MOVE_LOG_SEED, &game_id.to_le_bytes()], 
        bump
    )]
    pub move_log: Account<'info, MoveLog>,
    /// CHECK: PDA for escrowing SOL.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateGame>,
    game_id: u64,
    wager_amount: u64,
    game_type: GameType,
    match_type: MatchType,
    country: String,
    time_per_move: u16,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    game.game_id = game_id;
    game.white = ctx.accounts.player.key();
    game.black = match game_type {
        GameType::PvAI => crate::constants::ai_authority::ID,
        GameType::PvP => Pubkey::default(),
    };
    game.status = match game_type {
        GameType::PvAI => GameStatus::Active,
        GameType::PvP => GameStatus::WaitingForOpponent,
    };
    game.result = GameResult::None;
    game.fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string();
    game.move_count = 0;
    game.turn = 1;
    game.created_at = Clock::get()?.unix_timestamp;
    game.updated_at = game.created_at;
    game.wager_amount = wager_amount;
    game.wager_token = None;
    game.game_type = game_type;
    game.match_type = match_type;
    game.country_fee = get_country_fee(&country, match_type);
    game.time_per_move = time_per_move;
    game.bump = ctx.bumps.game;

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
        "Game {} created. Type: {:?}. Wager: {} SOL",
        game_id,
        game_type,
        wager_amount
    );
    Ok(())
}
