//! Session-signed variant of `join_game` for tournament play.
//!
//! Uses the tournament-scoped session key to co-sign joining a game,
//! drawing funds from the delegation PDA vault for cross-border fees and wagers.

use crate::account_ix::session_guards;
use crate::common::escrow::debit_program_pda;
use crate::constants::*;
use crate::errors::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64, game_id: u64)]
pub struct SessionJoinGame<'info> {
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

    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Box<Account<'info, Game>>,

    /// CHECK: PDA for escrowing SOL.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,

    /// White player profile for cross-border fee calculation.
    #[account(seeds = [PROFILE_SEED, game.white.as_ref()], bump)]
    pub white_profile: Box<Account<'info, PlayerProfile>>,

    /// Black player profile (the joining player).
    #[account(seeds = [PROFILE_SEED, player.key().as_ref()], bump)]
    pub player_profile: Box<Account<'info, PlayerProfile>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<SessionJoinGame>, _tournament_id: u64, _game_id: u64) -> Result<()> {
    let delegation = &mut ctx.accounts.session_delegation;
    let game = &mut ctx.accounts.game;
    let player_key = ctx.accounts.player.key();

    let now = Clock::get()?.unix_timestamp;
    require!(delegation.is_valid(now), XfchessGameError::SessionExpired);
    require!(
        delegation.session_key == ctx.accounts.session_signer.key(),
        XfchessGameError::SessionNotAuthorized
    );

    require!(
        game.game_type == GameType::PvP,
        GameErrorCode::GameAlreadyFull
    );
    require!(
        game.status == GameStatus::WaitingForOpponent,
        GameErrorCode::GameAlreadyFull
    );
    require!(game.white != player_key, GameErrorCode::CannotPlaySelf);

    // Platform fee was set at game creation time (universal, live-price-based).
    let final_fee = game.country_fee;

    let total_cost = game
        .wager_amount
        .checked_add(final_fee)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;

    require!(
        game.wager_amount <= delegation.max_wager,
        XfchessGameError::WagerLimitExceeded
    );
    require!(
        session_guards::checked_session_total(delegation.total_spent, total_cost)?
            <= delegation.spending_limit,
        XfchessGameError::SessionSpendingLimitExceeded
    );

    game.black = player_key;
    game.status = GameStatus::Active;
    game.updated_at = Clock::get()?.unix_timestamp;
    game.country_fee = final_fee;

    // Transfer wager from delegation PDA vault to escrow. `delegation` is a
    // program-owned PDA carrying real account data, so the System Program
    // refuses to act as `from` for it in any CPI ("Transfer: `from` must not
    // carry data") regardless of signing — `debit_program_pda` moves the
    // lamports directly instead. See `game_ix::global_create` for the fuller
    // writeup (this is the tournament join-side half of the same bug).
    if game.wager_token.is_none() {
        debit_program_pda(
            &delegation.to_account_info(),
            &ctx.accounts.escrow_pda.to_account_info(),
            game.wager_amount,
        )?;
    }

    // Update delegation spending (count wager + fee)
    delegation.total_spent =
        session_guards::checked_session_total(delegation.total_spent, total_cost)?;
    delegation.games_played = delegation.games_played.saturating_add(1);

    Ok(())
}
