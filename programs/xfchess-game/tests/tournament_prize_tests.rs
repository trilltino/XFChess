use anchor_lang::prelude::*;
use xfchess_game::state::{
    get_default_prize_shares, PayoutType, Tournament, TournamentStatus, TournamentType,
};
use xfchess_game::tournament_ix::prizes::ledger;

fn tournament(winner: Pubkey, prizes_claimed: u16) -> Tournament {
    Tournament {
        tournament_id: 1,
        authority: Pubkey::new_unique(),
        name: String::new(),
        entry_fee: 0,
        platform_fee: 0,
        prize_pool: 1_000_000,
        max_players: 8,
        player_count: 0,
        num_registered_players: 0,
        status: TournamentStatus::Completed,
        start_time: None,
        end_time: None,
        fees_advanced: 0,
        fee_payer: Pubkey::new_unique(),
        tournament_type: TournamentType::SingleElimination,
        current_round: 0,
        total_rounds: 0,
        total_matches: 7,
        final_match_index: 6,
        elo_min: 0,
        elo_max: 4000,
        min_players: 2,
        winner: Some(winner),
        second_place: None,
        third_place: None,
        fourth_place: None,
        fifth_place: None,
        sixth_place: None,
        seventh_place: None,
        eighth_place: None,
        ninth_place: None,
        tenth_place: None,
        prize_shares: [10_000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        created_at: 0,
        started_at: None,
        completed_at: None,
        bump: 0,
        prizes_claimed,
        platform_fee_pool: 0,
        usdc_prize_mint: None,
        usdc_prize_pool: 0,
        usdc_prize_funded: false,
        host_treasury: Pubkey::new_unique(),
        prize_token_mint: None,
        payout_type: PayoutType::LumpSum,
        vesting_params: None,
        base_time_seconds: 0,
        increment_seconds: 0,
        winner_takes_all: false,
    }
}

#[test]
fn prize_ledger_tracks_place_bits_and_amounts() {
    let winner = Pubkey::new_unique();
    let t = tournament(winner, 0);

    assert_eq!(ledger::find_place(&t, winner), Some((0, 10_000)));
    assert_eq!(ledger::place_bit(0).unwrap(), 1);
    assert_eq!(ledger::prize_amount(1_000_000, 6000).unwrap(), 600_000);
    assert!(ledger::funded_place_unclaimed(&t, 0).unwrap());

    let t = tournament(winner, 1);
    assert!(!ledger::funded_place_unclaimed(&t, 0).unwrap());
}

#[test]
fn default_prize_shares_keep_expected_boundaries() {
    assert_eq!(get_default_prize_shares(64, false)[..3], [6000, 3000, 1000]);
    assert_eq!(
        get_default_prize_shares(128, false)[..5],
        [5000, 2500, 1500, 500, 500]
    );
    assert_eq!(get_default_prize_shares(8, true)[0], 10_000);
}
