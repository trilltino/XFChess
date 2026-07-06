use anchor_lang::{prelude::*, AccountSerialize};
use xfchess_game::account_ix::profile_init;
use xfchess_game::elo::rating;
use xfchess_game::state::{PlayerProfile, PlayerSession};

#[test]
fn external_elo_is_stored_in_centiscale() {
    assert_eq!(rating::external_to_centiscale(1800).unwrap(), 180_000);
    assert_eq!(rating::centiscale_to_display(180_050.0), 1801);
    assert!(rating::external_to_centiscale(99).is_err());
    assert!(rating::external_to_centiscale(4001).is_err());
}

#[test]
fn profile_init_preserves_existing_stats_and_verification() {
    let player = Pubkey::new_unique();
    let mut existing = PlayerProfile::default();
    existing.authority = player;
    existing.wins = 7;
    existing.losses = 3;
    existing.elo_rating = 175_000.0;
    existing.is_verified = true;
    existing.lichess_verified = true;
    existing.lichess_blitz = 180_000;

    let mut data = Vec::new();
    existing.try_serialize(&mut data).unwrap();

    let mut loaded = profile_init::load_or_new_profile(&data, player, 1234).unwrap();
    profile_init::update_identity_fields(&mut loaded, "xfplayer".to_string(), "GB".to_string(), 1);

    assert_eq!(loaded.wins, 7);
    assert_eq!(loaded.losses, 3);
    assert_eq!(loaded.elo_rating, 175_000.0);
    assert!(loaded.is_verified);
    assert!(loaded.lichess_verified);
    assert_eq!(loaded.lichess_blitz, 180_000);
    assert_eq!(loaded.username, "xfplayer");
}

#[test]
fn player_session_budget_is_overflow_safe() {
    let session = PlayerSession {
        player: Pubkey::new_unique(),
        session_key: Pubkey::new_unique(),
        expires_at: 100,
        spending_limit: u64::MAX,
        total_spent: u64::MAX,
        max_wager: u64::MAX,
        can_create_games: true,
        can_join_games: true,
        can_claim_prizes: false,
        games_played: 0,
        is_active: true,
        bump: 0,
    };

    assert!(!session.has_budget(1));
}
