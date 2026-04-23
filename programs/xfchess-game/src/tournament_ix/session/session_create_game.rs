//! Session-signed variant of `create_game` for tournament play.
//!
//! Uses the tournament-scoped session key to co-sign game creation,
//! drawing funds from the delegation PDA vault for rent and wagers.

use crate::constants::*;
use crate::errors::*;
use crate::state::*;
use anchor_lang::prelude::*;

fn get_country_fee(country: &str, match_type: MatchType) -> u64 {
    if match_type == MatchType::Free {
        return 0;
    }
    match country {
        "GB" => UK_FEE_LAMPORTS,
        "BR" => BRAZIL_FEE_LAMPORTS,
        "CA" => CANADA_FEE_LAMPORTS,
        "DE" => GERMANY_FEE_LAMPORTS,
        _ => 0,
    }
}

#[derive(Accounts)]
#[instruction(
    tournament_id: u64,
    game_id: u64,
    wager_amount: u64,
    game_type: GameType,
    match_type: MatchType,
    country: String,
    time_per_move: u16
)]
pub struct SessionCreateGame<'info> {
    #[account(
        seeds = [b"tournament", tournament_id.to_le_bytes().as_ref()],
        bump = tournament.bump,
    )]
    pub tournament: Box<Account<'info, Tournament>>,

    #[account(
        mut,
        seeds = [
            TournamentSessionDelegation::SEED,
            tournament_id.to_le_bytes().as_ref(),
            player.key().as_ref(),
        ],
        bump = session_delegation.bump,
        constraint = session_delegation.enabled @ XfchessGameError::SessionNotAuthorized,
        constraint = session_delegation.player == player.key() @ XfchessGameError::UnauthorizedAccess,
    )]
    pub session_delegation: Box<Account<'info, TournamentSessionDelegation>>,

    /// Session key signer (hot key, not the player wallet).
    pub session_signer: Signer<'info>,

    #[account(
        constraint = tournament.players.iter().any(|p| p == player.key) @ XfchessGameError::UnauthorizedAccess,
    )]
    /// CHECK: Verified against tournament player list and delegation PDA.
    pub player: UncheckedAccount<'info>,

    #[account(
        init,
        payer = session_delegation,
        space = 8 + Game::INIT_SPACE,
        seeds = [GAME_SEED, &game_id.to_le_bytes()],
        bump
    )]
    pub game: Box<Account<'info, Game>>,

    #[account(
        init,
        payer = session_delegation,
        space = 10240,
        seeds = [MOVE_LOG_SEED, &game_id.to_le_bytes()],
        bump
    )]
    pub move_log: Box<Account<'info, MoveLog>>,

    /// CHECK: PDA for escrowing SOL wager.
    #[account(
        mut,
        seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<SessionCreateGame>,
    _tournament_id: u64,
    game_id: u64,
    wager_amount: u64,
    game_type: GameType,
    match_type: MatchType,
    country: String,
    time_per_move: u16,
) -> Result<()> {
    let delegation = &mut ctx.accounts.session_delegation;
    let game = &mut ctx.accounts.game;

    let now = Clock::get()?.unix_timestamp;
    require!(delegation.is_valid(now), XfchessGameError::SessionExpired);
    require!(
        delegation.session_key == ctx.accounts.session_signer.key(),
        XfchessGameError::SessionNotAuthorized
    );

    require!(wager_amount <= delegation.max_wager, XfchessGameError::WagerExceedsSessionCap);

    let rent = Rent::get()?;
    let game_rent = rent.minimum_balance(8 + Game::INIT_SPACE);
    let move_log_rent = rent.minimum_balance(10240);
    let total_cost = game_rent.saturating_add(move_log_rent).saturating_add(wager_amount);

    require!(
        delegation.total_spent.saturating_add(total_cost) <= delegation.spending_limit,
        XfchessGameError::SessionSpendingLimitExceeded
    );

    // Initialize game
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
    game.time_per_move = i64::from(time_per_move);
    game.bump = ctx.bumps.game;

    require!(wager_amount <= MAX_WAGER_AMOUNT, GameErrorCode::WagerTooHigh);

    // Transfer wager from delegation PDA vault to escrow
    if wager_amount > 0 {
        let tid_bytes = delegation.tournament_id.to_le_bytes();
        let player_bytes = delegation.player.to_bytes();
        let bump = [delegation.bump];
        let delegation_seeds: [&[u8]; 4] = [
            TournamentSessionDelegation::SEED,
            tid_bytes.as_ref(),
            player_bytes.as_ref(),
            bump.as_ref(),
        ];
        let signer_seeds: &[&[&[u8]]] = &[&delegation_seeds];

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: delegation.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
                signer_seeds,
            ),
            wager_amount,
        )?;
    }

    // Initialize move_log
    let move_log = &mut ctx.accounts.move_log;
    move_log.game_id = game_id;
    move_log.moves = Vec::new();
    move_log.timestamps = Vec::new();
    move_log.player_signatures = Vec::new();
    move_log.nonce = 0;

    // Update delegation spending
    delegation.total_spent = delegation.total_spent.saturating_add(total_cost);
    delegation.games_played = delegation.games_played.saturating_add(1);

    msg!(
        "Session-created game {} in tournament {}. Wager: {} SOL",
        game_id,
        delegation.tournament_id,
        wager_amount
    );
    Ok(())
}
