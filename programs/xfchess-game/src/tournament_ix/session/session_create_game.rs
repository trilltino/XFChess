//! Session-signed variant of `create_game` for tournament play.
//!
//! Uses the tournament-scoped session key to co-sign game creation,
//! drawing funds from the delegation PDA vault for rent and wagers.

use crate::account_ix::session_guards;
use crate::constants::*;
use crate::errors::*;
use crate::game_ix::common::{init_game_fields, InitGameArgs};
use crate::state::*;
use anchor_lang::prelude::*;

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

    /// TournamentPlayersShard 0 (players 0-63)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Box<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 1 (players 64-127)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Box<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 2 (players 128-191)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Box<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 3 (players 192-255)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Box<Account<'info, TournamentPlayersShard>>,

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
                || tournament_players_shard_1.players.iter().any(|p| *p == player.key())
                || tournament_players_shard_2.players.iter().any(|p| *p == player.key())
                || tournament_players_shard_3.players.iter().any(|p| *p == player.key())
        } @ XfchessGameError::UnauthorizedAccess,
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

    // Initialize game (full init, matching create/global_create — otherwise the
    // board, turn, and timestamps would be left zeroed and the game unplayable).
    let now = Clock::get()?.unix_timestamp;
    init_game_fields(
        &mut ctx.accounts.game,
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

    // Transfer wager from session vault to escrow
    if wager_amount > 0 {
        require!(
            session_guards::checked_session_total(session.total_spent, wager_amount)?
                <= session.spending_limit,
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
                System::id(),
                anchor_lang::system_program::Transfer {
                    from: session.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
                signer_seeds,
            ),
            wager_amount,
        )?;
    }

    // Update session spent amount
    let session_account = &mut ctx.accounts.session_delegation;
    session_account.total_spent =
        session_guards::checked_session_total(session_account.total_spent, wager_amount)?;

    Ok(())
}
