//! Shared helpers for tournament player shards.

use crate::errors::GameErrorCode;
use crate::state::{SwissStanding, TournamentPlayersShard};
use anchor_lang::prelude::*;

/// Number of `TournamentPlayersShard` accounts needed for `max_players`
/// (each shard caps out at `TournamentPlayersShard::SHARD_CAPACITY`).
pub fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

/// True if `player` appears in any of the given shards.
pub fn contains_player(shards: &[&TournamentPlayersShard], player: Pubkey) -> bool {
    shards
        .iter()
        .any(|shard| shard.players.iter().any(|candidate| *candidate == player))
}

/// Locates `player` across shards, returning `(shard_index, player_index)`.
pub fn find_player(shards: &[&TournamentPlayersShard], player: Pubkey) -> Option<(usize, usize)> {
    shards.iter().enumerate().find_map(|(shard_index, shard)| {
        shard
            .players
            .iter()
            .position(|candidate| *candidate == player)
            .map(|player_index| (shard_index, player_index))
    })
}

/// Appends `player`/`elo` to a shard's parallel arrays. Fails if the shard is
/// at `SHARD_CAPACITY` or the arrays are already out of sync.
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

/// Removes the player/elo pair at `index` from a shard's parallel arrays,
/// shifting later entries down (never leaves a default-pubkey gap).
pub fn remove_player(shard: &mut TournamentPlayersShard, index: usize) -> Result<()> {
    require!(
        index < shard.players.len() && index < shard.player_elos.len(),
        GameErrorCode::InvalidTournamentStatus
    );
    shard.players.remove(index);
    shard.player_elos.remove(index);
    Ok(())
}

/// Flattens all shards into one `(player, elo)` list, preserving shard order.
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

/// Resets every shard's `swiss_standings` to a zeroed entry per registered
/// player — called once when a Swiss tournament starts.
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
