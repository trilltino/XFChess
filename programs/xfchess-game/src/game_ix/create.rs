use crate::constants::*;
use crate::state::*;
use crate::errors::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64, wager_amount: u64, game_type: GameType)]
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
    /// Player profile — created on first game if it doesn't exist yet.
    #[account(
        init_if_needed,
        payer = player,
        space = 8 + PlayerProfile::INIT_SPACE,
        seeds = [PROFILE_SEED, player.key().as_ref()],
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateGame>,
    game_id: u64,
    wager_amount: u64,
    game_type: GameType,
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

    // Bootstrap profile if this is the player's first game.
    let profile = &mut ctx.accounts.player_profile;
    if profile.elo == 0 {
        profile.authority = ctx.accounts.player.key();
        profile.elo = 1200;
    }

    msg!(
        "Game {} created. Type: {:?}. Wager: {} SOL",
        game_id,
        game_type,
        wager_amount
    );
    Ok(())
}
