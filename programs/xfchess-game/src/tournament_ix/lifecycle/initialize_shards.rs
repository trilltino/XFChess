//! Shard initialization instructions — one per tournament size tier.
//!
//! Tier      max_players   Shards   Rent saved vs. always-4
//! ───────── ───────────── ──────   ───────────────────────
//! Small     ≤ 64          1        ~0.102 SOL
//! Medium    ≤ 128         2        ~0.068 SOL
//! Large     256           4        —
//!
//! Call the appropriate instruction after `initialize_tournament`.
//! The handler validates that `tournament.max_players` matches the tier.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

// ── Small (≤ 64 players — 1 shard) ───────────────────────────────────────────

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct InitializeShardsSmall<'info> {
    #[account(
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority,
        constraint = tournament.max_players <= 64 @ GameErrorCode::InvalidGameStatus,
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentPlayersShard::space_for(),
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    #[account(
        mut,
        constraint = authority.key() == vps_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_small(ctx: Context<InitializeShardsSmall>, tournament_id: u64) -> Result<()> {
    require!(
        ctx.accounts.tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );
    let s = &mut ctx.accounts.tournament_players_shard_0;
    s.tournament_id = tournament_id;
    s.shard_id = 0;
    s.players = Vec::with_capacity(TournamentPlayersShard::SHARD_CAPACITY as usize);
    s.player_elos = Vec::with_capacity(TournamentPlayersShard::SHARD_CAPACITY as usize);
    s.swiss_standings = Vec::with_capacity(TournamentPlayersShard::SHARD_CAPACITY as usize);
    msg!(
        "Tournament {} shard 0 initialized (small, ≤64 players)",
        tournament_id
    );
    Ok(())
}

// ── Medium (≤ 128 players — 2 shards) ────────────────────────────────────────

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct InitializeShardsMedium<'info> {
    #[account(
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority,
        constraint = tournament.max_players > 64 @ GameErrorCode::InvalidGameStatus,
        constraint = tournament.max_players <= 128 @ GameErrorCode::InvalidGameStatus,
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentPlayersShard::space_for(),
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentPlayersShard::space_for(),
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Account<'info, TournamentPlayersShard>,
    #[account(
        mut,
        constraint = authority.key() == vps_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_medium(ctx: Context<InitializeShardsMedium>, tournament_id: u64) -> Result<()> {
    require!(
        ctx.accounts.tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );
    let init_shard = |s: &mut TournamentPlayersShard, id: u8| {
        s.tournament_id = tournament_id;
        s.shard_id = id;
        s.players = Vec::with_capacity(TournamentPlayersShard::SHARD_CAPACITY as usize);
        s.player_elos = Vec::with_capacity(TournamentPlayersShard::SHARD_CAPACITY as usize);
        s.swiss_standings = Vec::with_capacity(TournamentPlayersShard::SHARD_CAPACITY as usize);
    };
    init_shard(&mut ctx.accounts.tournament_players_shard_0, 0);
    init_shard(&mut ctx.accounts.tournament_players_shard_1, 1);
    msg!(
        "Tournament {} shards 0-1 initialized (medium, ≤128 players)",
        tournament_id
    );
    Ok(())
}

// ── Large (256 players — 4 shards) ───────────────────────────────────────────

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct InitializeTournamentShards<'info> {
    #[account(
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority,
        constraint = tournament.max_players == 256 @ GameErrorCode::InvalidGameStatus,
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentPlayersShard::space_for(),
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentPlayersShard::space_for(),
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Account<'info, TournamentPlayersShard>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentPlayersShard::space_for(),
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Account<'info, TournamentPlayersShard>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentPlayersShard::space_for(),
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Account<'info, TournamentPlayersShard>,
    #[account(
        mut,
        constraint = authority.key() == vps_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitializeTournamentShards>, tournament_id: u64) -> Result<()> {
    require!(
        ctx.accounts.tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );
    let init_shard = |s: &mut TournamentPlayersShard, id: u8| {
        s.tournament_id = tournament_id;
        s.shard_id = id;
        s.players = Vec::with_capacity(TournamentPlayersShard::SHARD_CAPACITY as usize);
        s.player_elos = Vec::with_capacity(TournamentPlayersShard::SHARD_CAPACITY as usize);
        s.swiss_standings = Vec::with_capacity(TournamentPlayersShard::SHARD_CAPACITY as usize);
    };
    init_shard(&mut ctx.accounts.tournament_players_shard_0, 0);
    init_shard(&mut ctx.accounts.tournament_players_shard_1, 1);
    init_shard(&mut ctx.accounts.tournament_players_shard_2, 2);
    init_shard(&mut ctx.accounts.tournament_players_shard_3, 3);
    msg!(
        "Tournament {} shards 0-3 initialized (large, 256 players)",
        tournament_id
    );
    Ok(())
}
