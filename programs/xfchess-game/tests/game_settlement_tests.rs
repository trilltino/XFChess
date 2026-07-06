use anchor_lang::prelude::*;
use xfchess_game::common::escrow;
use xfchess_game::constants::CREATE_GAME_COST;
use xfchess_game::game_ix::common::{init_game_fields, opponent_of, InitGameArgs};
use xfchess_game::moves_ix::apply::apply_recorded_move;
use xfchess_game::state::{Game, GameResult, GameStatus, GameType, MatchType};

fn blank_game() -> Game {
    Game {
        game_id: 0,
        white: Pubkey::default(),
        black: Pubkey::default(),
        status: GameStatus::Pending,
        last_move_timestamp: 0,
        fees_advanced: 0,
        fee_payer: Pubkey::default(),
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

#[test]
fn pot_is_wager_times_two_with_overflow_guard() {
    assert_eq!(escrow::pot(50).unwrap(), 100);
    assert!(escrow::pot(u64::MAX).is_err());
}

#[test]
fn init_game_fields_sets_complete_waiting_state() {
    let mut game = blank_game();
    let white = Pubkey::new_unique();
    let payer = Pubkey::new_unique();

    init_game_fields(
        &mut game,
        InitGameArgs {
            game_id: 42,
            white,
            fee_payer: payer,
            wager_amount: 123,
            match_type: MatchType::Rated,
            platform_fee: 7,
            base_time_seconds: 600,
            increment_seconds: 5,
            tournament_id: Some(9),
        },
        111,
        255,
    )
    .unwrap();

    assert_eq!(game.game_id, 42);
    assert_eq!(game.white, white);
    assert_eq!(game.black, Pubkey::default());
    assert_eq!(game.status, GameStatus::WaitingForOpponent);
    assert_eq!(game.result, GameResult::None);
    assert_eq!(game.created_at, 111);
    assert_eq!(game.updated_at, 111);
    assert_eq!(game.last_move_timestamp, 111);
    assert_eq!(game.fee_payer, payer);
    assert_eq!(game.fees_advanced, CREATE_GAME_COST);
    assert_eq!(game.tournament_id, Some(9));
}

#[test]
fn move_apply_rejects_turn_and_nonce_mismatches() {
    let mut game = blank_game();
    game.status = GameStatus::Active;
    game.white = Pubkey::new_unique();
    game.black = Pubkey::new_unique();
    game.nonce = 7;

    assert!(apply_recorded_move(&mut game, game.black, [0; 5], [0; 68], 8, Some(7), 1).is_err());
    assert!(apply_recorded_move(&mut game, game.white, [0; 5], [0; 68], 9, Some(7), 1).is_err());
}

#[test]
fn opponent_guard_returns_the_other_player() {
    let mut game = blank_game();
    game.white = Pubkey::new_unique();
    game.black = Pubkey::new_unique();
    assert_eq!(opponent_of(&game, game.white).unwrap(), game.black);
    assert!(opponent_of(&game, Pubkey::new_unique()).is_err());
}
