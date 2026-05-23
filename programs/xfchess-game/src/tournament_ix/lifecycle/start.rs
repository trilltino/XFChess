//! Instruction to lock registration and seed players for bracket generation.
//! Match accounts are created separately via initialize_match instructions.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct StartTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,
    /// TournamentPlayersShard 0 (players 0-63)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 1 (players 64-127)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 2 (players 128-191)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 3 (players 192-255)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Account<'info, TournamentPlayersShard>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Sorts players by ELO descending and records seed order.
/// Backend uses this to generate matches via separate initialize_match calls.
pub fn handler(ctx: Context<StartTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;

    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );
    require!(
        tournament.num_registered_players == tournament.max_players,
        GameErrorCode::TournamentFull
    );

    let player_count = tournament.num_registered_players as usize;

    // Collect all players and ELOs from all shards
    let mut all_players: Vec<Pubkey> = Vec::with_capacity(player_count);
    let mut all_elos: Vec<u32> = Vec::with_capacity(player_count);

    let shards = [
        &ctx.accounts.tournament_players_shard_0,
        &ctx.accounts.tournament_players_shard_1,
        &ctx.accounts.tournament_players_shard_2,
        &ctx.accounts.tournament_players_shard_3,
    ];

    for shard in shards.iter() {
        for i in 0..shard.players.len() {
            all_players.push(shard.players[i]);
            all_elos.push(shard.player_elos[i]);
        }
    }

    // Sort players by ELO descending
    let mut indexed: Vec<(usize, u32)> = all_elos
        .iter()
        .enumerate()
        .map(|(i, &elo)| (i, elo))
        .collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1));

    // Create sorted arrays
    let mut seeded_players: Vec<Pubkey> = Vec::with_capacity(player_count);
    let mut seeded_elos: Vec<u32> = Vec::with_capacity(player_count);
    for (original_idx, elo) in indexed {
        seeded_players.push(all_players[original_idx]);
        seeded_elos.push(elo);
    }

    // Redistribute sorted players back to shards in order
    let tp0 = &mut ctx.accounts.tournament_players_shard_0;
    let tp1 = &mut ctx.accounts.tournament_players_shard_1;
    let tp2 = &mut ctx.accounts.tournament_players_shard_2;
    let tp3 = &mut ctx.accounts.tournament_players_shard_3;

    // Clear all shards
    tp0.players.clear();
    tp0.player_elos.clear();
    tp1.players.clear();
    tp1.player_elos.clear();
    tp2.players.clear();
    tp2.player_elos.clear();
    tp3.players.clear();
    tp3.player_elos.clear();

    // Distribute sorted players across shards
    for (i, &player) in seeded_players.iter().enumerate() {
        let shard_id = (i / TournamentPlayersShard::SHARD_CAPACITY as usize) as u8;
        match shard_id {
            0 => {
                tp0.players.push(player);
                tp0.player_elos.push(seeded_elos[i]);
            }
            1 => {
                tp1.players.push(player);
                tp1.player_elos.push(seeded_elos[i]);
            }
            2 => {
                tp2.players.push(player);
                tp2.player_elos.push(seeded_elos[i]);
            }
            3 => {
                tp3.players.push(player);
                tp3.player_elos.push(seeded_elos[i]);
            }
            _ => return Err(GameErrorCode::TournamentFull.into()),
        }
    }

    tournament.status = TournamentStatus::Active;
    tournament.current_round = 0;
    tournament.started_at = Some(Clock::get()?.unix_timestamp);

    Ok(())
}
