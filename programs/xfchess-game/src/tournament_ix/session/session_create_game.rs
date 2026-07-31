//! Session-signed variant of `create_game` for tournament play.
//!
//! Uses the tournament-scoped session key to co-sign game creation,
//! drawing funds from the delegation PDA vault for rent and wagers.

use crate::account_ix::session_guards;
use crate::common::escrow::debit_program_pda;
use crate::constants::*;
use crate::errors::*;
use crate::game_ix::common::{init_game_fields, InitGameArgs};
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::solana_program::system_instruction;
use anchor_lang::Discriminator;

#[derive(Accounts)]
#[instruction(
    tournament_id: u64,
    game_id: u64,
    wager_amount: u64,
    match_type: MatchType,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16
)]
pub struct SessionCreateGame<'info> {
    #[account(
        seeds = [b"tournament", tournament_id.to_le_bytes().as_ref()],
        bump = tournament.bump,
    )]
    pub tournament: Box<Account<'info, Tournament>>,

    /// TournamentPlayersShard 0 always present (all tournament sizes).
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Box<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 1 — present for >64-player tournaments only.
    /// Pass the program ID in its place for smaller tournaments.
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Box<Account<'info, TournamentPlayersShard>>>,
    /// TournamentPlayersShard 2 — present for 256-player tournaments only.
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Box<Account<'info, TournamentPlayersShard>>>,
    /// TournamentPlayersShard 3 — present for 256-player tournaments only.
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Box<Account<'info, TournamentPlayersShard>>>,

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
        constraint = session_delegation.session_key == session_signer.key() @ XfchessGameError::InvalidSessionKey,
    )]
    pub session_delegation: Box<Account<'info, TournamentSessionDelegation>>,

    /// Session key signer (hot key, not the player wallet).
    pub session_signer: Signer<'info>,

    #[account(
        constraint = {
            tournament_players_shard_0.players.iter().any(|p| *p == player.key())
                || tournament_players_shard_1.as_ref().is_some_and(|s| s.players.iter().any(|p| *p == player.key()))
                || tournament_players_shard_2.as_ref().is_some_and(|s| s.players.iter().any(|p| *p == player.key()))
                || tournament_players_shard_3.as_ref().is_some_and(|s| s.players.iter().any(|p| *p == player.key()))
        } @ XfchessGameError::UnauthorizedAccess,
    )]
    /// CHECK: Verified against tournament player list and delegation PDA.
    pub player: UncheckedAccount<'info>,

    /// CHECK: Created manually in the handler — see the identical comment in
    /// `game_ix::global_create::GlobalCreateGame::game` for why `init, payer
    /// = session_delegation` cannot work when the payer is a PDA.
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: UncheckedAccount<'info>,

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
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<()> {
    let session = &ctx.accounts.session_delegation;
    let tournament = &ctx.accounts.tournament;
    let _escrow_pda = &ctx.accounts.escrow_pda;
    let fee_payer = &ctx.accounts.session_signer;
    let _system_program = &ctx.accounts.system_program;

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
        session_guards::checked_session_total(session.total_spent, wager_amount)?
            <= session.spending_limit,
        GameErrorCode::SpendingLimitExceeded
    );

    // Check wager limits
    require!(
        wager_amount <= session.max_wager,
        GameErrorCode::WagerLimitExceeded
    );
    require!(
        wager_amount == 0 || wager_amount >= MIN_WAGER_LAMPORTS,
        GameErrorCode::StakeTooLow
    );

    // Validate tournament state
    require!(
        tournament.status == TournamentStatus::Active,
        GameErrorCode::InvalidTournamentStatus
    );

    let now = Clock::get()?.unix_timestamp;

    // Create the `game` PDA funded by the tournament session delegation
    // vault via `debit_program_pda`, not a system-program CPI — see the
    // comment on `game_ix::global_create::handler` for why: the System
    // Program's transfer path (which `create_account` also uses internally)
    // unconditionally rejects a `from` account that carries data, and
    // `session_delegation` does. `Allocate` + `Assign` handle space/ownership
    // separately since neither touches a `from` account.
    //
    // Order matters: `Allocate`+`Assign` must run *before* the debit —
    // crediting `game` first (while still owned by the System Program) then
    // passing it into `invoke_signed` trips the runtime's "sum of account
    // balances before and after instruction do not match" check. See
    // `game_ix::global_create`'s test for the empirical confirmation.
    let space = 8 + Game::INIT_SPACE;
    let lamports = Rent::get()?.minimum_balance(space);
    let game_id_bytes = game_id.to_le_bytes();
    let game_bump = [ctx.bumps.game];
    let game_seeds: [&[u8]; 3] = [GAME_SEED, game_id_bytes.as_ref(), game_bump.as_ref()];

    invoke_signed(
        &system_instruction::allocate(&ctx.accounts.game.key(), space as u64),
        &[
            ctx.accounts.game.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[&game_seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(&ctx.accounts.game.key(), ctx.program_id),
        &[
            ctx.accounts.game.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[&game_seeds],
    )?;
    debit_program_pda(
        &ctx.accounts.session_delegation.to_account_info(),
        &ctx.accounts.game.to_account_info(),
        lamports,
    )?;

    // Transfer wager from session vault to escrow — same rule.
    if wager_amount > 0 {
        require!(
            session_guards::checked_session_total(session.total_spent, wager_amount)?
                <= session.spending_limit,
            GameErrorCode::InsufficientFunds
        );
    }
    debit_program_pda(
        &ctx.accounts.session_delegation.to_account_info(),
        &ctx.accounts.escrow_pda.to_account_info(),
        wager_amount,
    )?;

    // Update session spent amount
    let session_account = &mut ctx.accounts.session_delegation;
    session_account.total_spent =
        session_guards::checked_session_total(session_account.total_spent, wager_amount)?;

    // Initialize game (full init, matching create/global_create — otherwise the
    // board, turn, and timestamps would be left zeroed and the game unplayable).
    let mut game = Game {
        game_id: 0,
        white: Pubkey::default(),
        black: Pubkey::default(),
        status: GameStatus::Pending,
        last_move_timestamp: 0,
        fees_advanced: 0,
        fee_payer: Pubkey::default(),
        result: GameResult::None,
        board_state: [0; 68],
        move_count: 0,
        halfmove_clock: 0,
        turn: 0,
        created_at: 0,
        updated_at: 0,
        wager_amount: 0,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Free,
        country_fee: 0,
        base_time_seconds: 0,
        increment_seconds: 0,
        bump: 0,
        is_delegated: false,
        tournament_id: None,
        nonce: 0,
    };
    init_game_fields(
        &mut game,
        InitGameArgs {
            game_id,
            white: ctx.accounts.player.key(),
            fee_payer: fee_payer.key(),
            wager_amount,
            match_type,
            platform_fee,
            base_time_seconds,
            increment_seconds,
            tournament_id: Some(tournament_id),
        },
        now,
        ctx.bumps.game,
    )?;

    let mut data = ctx.accounts.game.try_borrow_mut_data()?;
    data[..8].copy_from_slice(&Game::DISCRIMINATOR);
    let mut writer = &mut data[8..];
    game.serialize(&mut writer)?;

    Ok(())
}
