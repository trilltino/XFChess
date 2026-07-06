use anchor_lang::prelude::*;
use xfchess_game::state::TournamentPlayersShard;
use xfchess_game::tournament_ix::shards;

fn shard(id: u8) -> TournamentPlayersShard {
    TournamentPlayersShard {
        tournament_id: 1,
        shard_id: id,
        players: Vec::new(),
        player_elos: Vec::new(),
        swiss_standings: Vec::new(),
    }
}

#[test]
fn required_shards_matches_capacity_boundaries() {
    assert_eq!(shards::required_shards(64), 1);
    assert_eq!(shards::required_shards(65), 2);
    assert_eq!(shards::required_shards(129), 4);
}

#[test]
fn remove_player_shrinks_vectors_without_default_entries() {
    let mut shard = shard(0);
    let p0 = Pubkey::new_unique();
    let p1 = Pubkey::new_unique();
    shards::push_player(&mut shard, p0, 1200).unwrap();
    shards::push_player(&mut shard, p1, 1300).unwrap();

    shards::remove_player(&mut shard, 0).unwrap();

    assert_eq!(shard.players, vec![p1]);
    assert_eq!(shard.player_elos, vec![1300]);
    assert!(!shard.players.contains(&Pubkey::default()));
}

#[test]
fn swiss_standings_initialize_from_registered_players() {
    let mut shard = shard(0);
    let p0 = Pubkey::new_unique();
    let p1 = Pubkey::new_unique();
    shards::push_player(&mut shard, p0, 1200).unwrap();
    shards::push_player(&mut shard, p1, 1300).unwrap();

    shards::initialize_swiss_standings(&mut [&mut shard]).unwrap();

    assert_eq!(shard.swiss_standings.len(), 2);
    assert_eq!(shard.swiss_standings[0].player, p0);
    assert_eq!(shard.swiss_standings[1].score, 0);
}
