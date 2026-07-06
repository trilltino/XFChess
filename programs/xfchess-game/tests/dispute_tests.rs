use anchor_lang::prelude::*;
use xfchess_game::governance_ix::resolution;
use xfchess_game::state::{
    DisputeRecord, DisputeStatus, Game, GameResult, GameStatus, GameType, MatchType,
};

fn game() -> Game {
    Game {
        game_id: 1,
        white: Pubkey::new_unique(),
        black: Pubkey::new_unique(),
        status: GameStatus::Disputed,
        last_move_timestamp: 0,
        fees_advanced: 0,
        fee_payer: Pubkey::new_unique(),
        result: GameResult::None,
        board_state: [0; 68],
        move_count: 0,
        halfmove_clock: 0,
        turn: 1,
        created_at: 0,
        updated_at: 0,
        wager_amount: 0,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Free,
        country_fee: 0,
        base_time_seconds: 0,
        increment_seconds: 0,
        bump: 0,
        is_delegated: false,
        tournament_id: None,
        nonce: 0,
    }
}

fn dispute() -> DisputeRecord {
    DisputeRecord {
        game_id: 1,
        challenger: Pubkey::new_unique(),
        reason: "lag".to_string(),
        evidence_hash: [0; 32],
        status: DisputeStatus::Pending,
        resolved_by: None,
        resolution: String::new(),
        created_at: 0,
        expires_at: 100,
        resolved_at: None,
        bond_amount: 0,
        bump: 0,
    }
}

#[test]
fn resolution_rejects_non_player_winner() {
    let game = game();
    assert!(resolution::validate_resolution(&game, Some(Pubkey::new_unique())).is_err());
    assert_eq!(
        resolution::validate_resolution(&game, Some(game.white)).unwrap(),
        GameResult::Winner(game.white)
    );
    assert_eq!(
        resolution::validate_resolution(&game, None).unwrap(),
        GameResult::Draw
    );
}

#[test]
fn apply_resolution_sets_dispute_and_game_terminal_state() {
    let mut game = game();
    let mut dispute = dispute();
    let authority = Pubkey::new_unique();
    resolution::apply_resolution(
        &mut game,
        &mut dispute,
        GameResult::Draw,
        "draw accepted".to_string(),
        authority,
        55,
    )
    .unwrap();

    assert!(dispute.status == DisputeStatus::Resolved);
    assert_eq!(dispute.resolved_by, Some(authority));
    assert_eq!(dispute.resolved_at, Some(55));
    assert_eq!(game.status, GameStatus::Settled);
    assert_eq!(game.result, GameResult::Draw);
}
