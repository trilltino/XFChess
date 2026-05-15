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
    match_type: MatchType,
    country: String,
    base_time_seconds: u64,
    increment_seconds: u16
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
        constraint = tournament.players.iter().any(|p| *p == player.key()) @ XfchessGameError::UnauthorizedAccess,
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
        seeds = [MOVE_LOG_SEED, &game_id.to_le_bytes()],
        bump,
        space = MoveLog::LEN
    )]
    pub move_log: Account<'info, MoveLog>,

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
    tournament_id: u64,
    game_id: u64,
    wager_amount: u64,
    match_type: MatchType,
    country: String,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<()> {
    let session = &ctx.accounts.session_delegation;
    let tournament = &ctx.accounts.tournament;
    let game = &mut ctx.accounts.game;
    let move_log = &mut ctx.accounts.move_log;
    let _escrow_pda = &ctx.accounts.escrow_pda;
    let fee_payer = &ctx.accounts.session_signer;
    let _system_program = &ctx.accounts.system_program;
    let rent = &Rent::get()?;

    // Validate session
    require!(
        session.tournament_id == tournament_id,
        GameErrorCode::InvalidSession
    );
    require!(
        session.is_valid(Clock::get()?.unix_timestamp),
        GameErrorCode::SessionExpired
    );
    require!(
        session.total_spent.saturating_add(wager_amount) <= session.spending_limit,
        GameErrorCode::SpendingLimitExceeded
    );

    // Check wager limits
    require!(
        wager_amount <= session.max_wager,
        GameErrorCode::WagerLimitExceeded
    );

    // Validate tournament state
    require!(
        tournament.status == TournamentStatus::Active,
        GameErrorCode::InvalidTournamentStatus
    );

    // Initialize game
    game.game_id = game_id;
    game.white = ctx.accounts.player.key();
    game.black = Pubkey::default();
    game.status = GameStatus::WaitingForOpponent;
    game.match_type = match_type;
    game.country_fee = get_country_fee(&country, match_type);
    game.wager_amount = wager_amount;
    game.base_time_seconds = base_time_seconds;
    game.increment_seconds = increment_seconds;
    game.fee_payer = fee_payer.key();

    // Initialize move log
    move_log.game_id = game_id;
    move_log.moves = Vec::new();
    move_log.timestamps = Vec::new();
    move_log.player_signatures = Vec::new();
    move_log.nonce = 0;

    // Transfer wager from session vault to escrow
    if wager_amount > 0 {
        require!(
            session.total_spent + wager_amount <= session.spending_limit,
            GameErrorCode::InsufficientFunds
        );
        let tid_bytes = session.tournament_id.to_le_bytes();
        let player_bytes = session.player.to_bytes();
        let bump = [session.bump];
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
                    from: session.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
                signer_seeds,
            ),
            wager_amount,
        )?;
    }

    // Calculate and transfer rent for move_log
    let move_log_rent = rent.minimum_balance(MoveLog::LEN);
    require!(
        session.total_spent + wager_amount + move_log_rent <= session.spending_limit,
        GameErrorCode::SpendingLimitExceeded
    );
    let tid_bytes = session.tournament_id.to_le_bytes();
    let player_bytes = session.player.to_bytes();
    let bump = [session.bump];
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
                from: session.to_account_info(),
                to: ctx.accounts.move_log.to_account_info(),
            },
            signer_seeds,
        ),
        move_log_rent,
    )?;

    // Update session spent amount
    let session_account = &mut ctx.accounts.session_delegation;
    session_account.total_spent += wager_amount + move_log_rent;

    msg!("Session created game {} in tournament {}", game_id, tournament_id);
    Ok(())
}
