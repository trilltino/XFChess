//! Shared helpers for tournament player shards.

use crate::errors::GameErrorCode;
use crate::state::{SwissStanding, TournamentPlayersShard};
use anchor_lang::prelude::*;

pub fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

pub fn contains_player(shards: &[&TournamentPlayersShard], player: Pubkey) -> bool {
    shards
        .iter()
        .any(|shard| shard.players.iter().any(|candidate| *candidate == player))
}

pub fn find_player(shards: &[&TournamentPlayersShard], player: Pubkey) -> Option<(usize, usize)> {
    shards.iter().enumerate().find_map(|(shard_index, shard)| {
        shard
            .players
            .iter()
            .position(|candidate| *candidate == player)
            .map(|player_index| (shard_index, player_index))
    })
}

pub fn push_player(shard: &mut TournamentPlayersShard, player: Pubkey, elo: u32) -> Result<()> {
    require!(
        shard.players.len() < TournamentPlayersShard::SHARD_CAPACITY as usize,
        GameErrorCode::TournamentFull
    );
    require!(
        shard.players.len() == shard.player_elos.len(),
        GameErrorCode::InvalidTournamentStatus
    );
    shard.players.push(player);
    shard.player_elos.push(elo);
    Ok(())
}

pub fn remove_player(shard: &mut TournamentPlayersShard, index: usize) -> Result<()> {
    require!(
        index < shard.players.len() && index < shard.player_elos.len(),
        GameErrorCode::InvalidTournamentStatus
    );
    shard.players.remove(index);
    shard.player_elos.remove(index);
    Ok(())
}

pub fn collect_players(shards: &[&TournamentPlayersShard]) -> Result<Vec<(Pubkey, u32)>> {
    let mut out = Vec::new();
    for shard in shards {
        require!(
            shard.players.len() == shard.player_elos.len(),
            GameErrorCode::InvalidTournamentStatus
        );
        out.extend(
            shard
                .players
                .iter()
                .copied()
                .zip(shard.player_elos.iter().copied()),
        );
    }
    Ok(out)
}

pub fn initialize_swiss_standings(shards: &mut [&mut TournamentPlayersShard]) -> Result<()> {
    for shard in shards.iter_mut() {
        require!(
            shard.players.len() == shard.player_elos.len(),
            GameErrorCode::InvalidTournamentStatus
        );
        shard.swiss_standings.clear();
        for player in shard.players.iter().copied() {
            shard.swiss_standings.push(SwissStanding {
                player,
                score: 0,
                buchholz: 0,
                sonneborn: 0,
                color_balance: 0,
            });
        }
    }
    Ok(())
}
