use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::game_ix::common::{init_game_fields, InitGameArgs};
use crate::state::*;
use anchor_lang::prelude::*;

/// Seats a tournament pairing in a ready-to-play Game, signed by the tournament
/// authority — the same key that reports tournament results. Players sign nothing
/// after registration: the Game starts Active with both seats filled, and each
/// player gets the per-game `SessionDelegation` that `record_move` checks, pointing
/// at `session_key` (the backend-held key that also pays the Game's fees, as in
/// any other game).
#[derive(Accounts)]
#[instruction(tournament_id: u64, game_id: u64)]
pub struct CreateTournamentGame<'info> {
    #[account(
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Box<Account<'info, TournamentPlayersShard>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Box<Account<'info, TournamentPlayersShard>>>,

    #[account(
        init,
        payer = authority,
        space = 8 + Game::INIT_SPACE,
        seeds = [GAME_SEED, &game_id.to_le_bytes()],
        bump
    )]
    pub game: Box<Account<'info, Game>>,

    #[account(
        init,
        payer = authority,
        seeds = [b"session_delegation", game_id.to_le_bytes().as_ref(), white.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<SessionDelegation>() + 8
    )]
    pub white_session: Box<Account<'info, SessionDelegation>>,

    #[account(
        init,
        payer = authority,
        seeds = [b"session_delegation", game_id.to_le_bytes().as_ref(), black.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<SessionDelegation>() + 8
    )]
    pub black_session: Box<Account<'info, SessionDelegation>>,

    /// CHECK: only its key is used; it must be a registered player.
    pub white: UncheckedAccount<'info>,
    /// CHECK: only its key is used; it must be a registered player.
    pub black: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// How long the per-game move delegation lives: two hours plus both clocks, so a
/// slow classical game cannot outlive its own session.
fn session_lifetime(base_time_seconds: u64) -> i64 {
    let clocks = i64::try_from(base_time_seconds.saturating_mul(2)).unwrap_or(i64::MAX / 2);
    (2 * 60 * 60_i64).saturating_add(clocks)
}

pub fn handler(
    ctx: Context<CreateTournamentGame>,
    tournament_id: u64,
    game_id: u64,
    session_key: Pubkey,
) -> Result<()> {
    let tournament = &ctx.accounts.tournament;
    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    require!(
        tournament.status == TournamentStatus::Active,
        GameErrorCode::InvalidTournamentStatus
    );

    let white = ctx.accounts.white.key();
    let black = ctx.accounts.black.key();
    require_keys_neq!(white, black, GameErrorCode::CannotPlaySelf);

    let mut shards: Vec<&TournamentPlayersShard> = vec![&ctx.accounts.tournament_players_shard_0];
    for shard in [
        &ctx.accounts.tournament_players_shard_1,
        &ctx.accounts.tournament_players_shard_2,
        &ctx.accounts.tournament_players_shard_3,
    ]
    .into_iter()
    .flatten()
    {
        shards.push(shard);
    }
    require!(
        crate::tournament_ix::shards::contains_player(&shards, white)
            && crate::tournament_ix::shards::contains_player(&shards, black),
        GameErrorCode::PlayerNotFound
    );

    let now = Clock::get()?.unix_timestamp;
    let base_time_seconds = tournament.base_time_seconds;
    let game = &mut ctx.accounts.game;
    init_game_fields(
        game,
        InitGameArgs {
            game_id,
            white,
            fee_payer: session_key,
            wager_amount: 0,
            match_type: MatchType::Free,
            platform_fee: 0,
            base_time_seconds,
            increment_seconds: tournament.increment_seconds,
            tournament_id: Some(tournament_id),
        },
        now,
        ctx.bumps.game,
    )?;
    crate::lifecycle::transitions::join_waiting_game(game, black, session_key, now)?;

    let expires_at = now.saturating_add(session_lifetime(base_time_seconds));
    for (session, player, bump) in [
        (
            &mut ctx.accounts.white_session,
            white,
            ctx.bumps.white_session,
        ),
        (
            &mut ctx.accounts.black_session,
            black,
            ctx.bumps.black_session,
        ),
    ] {
        session.game_id = game_id;
        session.player = player;
        session.session_key = session_key;
        session.expires_at = expires_at;
        session.max_batch_len = 10;
        session.enabled = true;
        session.bump = bump;
    }

    msg!(
        "Tournament {} game {} created: {} (W) vs {} (B)",
        tournament_id,
        game_id,
        white,
        black
    );
    Ok(())
}
