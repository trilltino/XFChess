use anchor_lang::InstructionData;
use anchor_lang::prelude::Pubkey;
use xfchess_game::{
    instruction::{CreateGame, InitProfile, InitializeTournament, JoinGame},
    state::{MatchType, TournamentSessionDelegation, TournamentType},
};

#[test]
fn init_profile_instruction_serializes() {
    let data = InitProfile {
        username: "tester123".to_string(),
        country: "US".to_string(),
    }
    .data();

    assert!(data.len() > 8);
}

#[test]
fn create_and_join_game_instructions_serialize() {
    let create_data = CreateGame {
        game_id: 42,
        wager_amount: 0,
        match_type: MatchType::Free,
        country: "US".to_string(),
        base_time_seconds: 300,
        increment_seconds: 3,
    }
    .data();

    let join_data = JoinGame { game_id: 42 }.data();

    assert!(create_data.len() > join_data.len());
}

#[test]
fn initialize_tournament_instruction_serializes() {
    let data = InitializeTournament {
        tournament_id: 7,
        name: "Weekend Cup".to_string(),
        entry_fee: 1_000_000,
        max_players: 16,
        tournament_type: TournamentType::SingleElimination,
        elo_min: 0,
        elo_max: 3000,
        min_players: 8,
        prize_shares: [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
        winner_takes_all: false,
        host_treasury: Pubkey::new_unique(),
        usdc_mint: None,
        base_time_seconds: 600,
        increment_seconds: 5,
    }
    .data();

    assert!(data.len() > 8);
}

#[test]
fn tournament_session_budget_enforced() {
    let mut delegation = TournamentSessionDelegation {
        tournament_id: 1,
        player: Pubkey::new_unique(),
        session_key: Pubkey::new_unique(),
        expires_at: 10_000,
        spending_limit: 1_000_000,
        total_spent: 400_000,
        max_wager: 300_000,
        games_played: 0,
        enabled: true,
        bump: 255,
    };

    assert!(delegation.is_valid(9_999));
    assert!(delegation.has_budget(300_000));
    assert!(!delegation.has_budget(300_001));

    delegation.total_spent = 800_000;
    assert!(!delegation.has_budget(250_000));
}
