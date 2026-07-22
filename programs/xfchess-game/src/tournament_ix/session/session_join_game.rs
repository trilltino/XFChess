//! Session-signed variant of `join_game` for tournament play.
//!
//! Uses the tournament-scoped session key to co-sign joining a game,
//! drawing funds from the delegation PDA vault for cross-border fees and wagers.

use crate::account_ix::session_guards;
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

    // Transfer wager from delegation PDA vault to escrow
    if game.wager_amount > 0 && game.wager_token.is_none() {
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
                System::id(),
                anchor_lang::system_program::Transfer {
                    from: delegation.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
                signer_seeds,
            ),
            game.wager_amount,
        )?;
    }

    // Update delegation spending (count wager + fee)
    delegation.total_spent =
        session_guards::checked_session_total(delegation.total_spent, total_cost)?;
    delegation.games_played = delegation.games_played.saturating_add(1);

    Ok(())
}
