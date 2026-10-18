mod common;

use anchor_lang::{InstructionData, Space, ToAccountMetas};
use common::*;
use solana_sdk::{
    account::AccountSharedData, clock::Clock, instruction::Instruction, pubkey::Pubkey,
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{Game, GameResult, GameStatus, GameType, MatchType};

const GAME_ID: u64 = 91_001;
const BASE_TIME_SECONDS: u64 = 300;
// ER validators produce slots far faster than base Solana; any long-running ER
// is well past this height.
const ER_SLOT: u64 = 50_000_000;

fn delegated_game(
    white: Pubkey,
    black: Pubkey,
    updated_at: i64,
) -> (Pubkey, solana_sdk::account::Account) {
    let (pda, bump) = game_pda(GAME_ID);
    let game = Game {
        game_id: GAME_ID,
        white,
        black,
        status: GameStatus::Active,
        last_move_timestamp: updated_at,
        fees_advanced: 0,
        fee_payer: white,
        result: GameResult::None,
        board_state: start_board(),
        move_count: 1,
        halfmove_clock: 0,
        turn: 2,
        created_at: updated_at,
        updated_at,
        wager_amount: 0,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Free,
        country_fee: 0,
        base_time_seconds: BASE_TIME_SECONDS,
        increment_seconds: 0,
        bump,
        is_delegated: true,
        tournament_id: None,
        nonce: 1,
        draw_offered_by: None,
    };
    (pda, program_account(&game, 8 + Game::INIT_SPACE))
}

fn crank_ix(white: Pubkey, black: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_crank_time_check::CrankTimeCheck {
        game: game_pda(GAME_ID).0,
        white,
        black,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::CrankTimeCheck {
        _data: xfchess_game::CrankTimeCheckData {},
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

async fn at_er_slot() -> (solana_program_test::ProgramTestContext, i64) {
    let mut ctx = start(vec![]).await;
    ctx.warp_to_slot(ER_SLOT).expect("warp");
    ctx.last_blockhash = ctx.get_new_latest_blockhash().await.expect("blockhash");
    let now = ctx
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .unwrap()
        .unix_timestamp;
    (ctx, now)
}

#[tokio::test]
async fn crank_resolves_an_expired_game_at_a_high_er_slot() {
    let (white, black) = (Pubkey::new_unique(), Pubkey::new_unique());
    let (mut ctx, now) = at_er_slot().await;

    // Black (to move) has been idle longer than a full 300s clock.
    let (key, account) = delegated_game(white, black, now - 400);
    ctx.set_account(&key, &AccountSharedData::from(account));

    send(&mut ctx, crank_ix(white, black), &[])
        .await
        .expect("a late crank must still end an expired game");

    let game = fetch_game(&mut ctx, GAME_ID).await;
    assert_eq!(game.status, GameStatus::Finished);
    assert_eq!(game.result, GameResult::Winner(white));
}

#[tokio::test]
async fn crank_rejects_a_run_well_before_the_window() {
    let (white, black) = (Pubkey::new_unique(), Pubkey::new_unique());
    let (mut ctx, now) = at_er_slot().await;

    let (key, account) = delegated_game(white, black, now - 10);
    ctx.set_account(&key, &AccountSharedData::from(account));

    let err = send(&mut ctx, crank_ix(white, black), &[])
        .await
        .expect_err("a crank long before the window must not run");
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::CrankTooEarly)));
    assert_eq!(
        fetch_game(&mut ctx, GAME_ID).await.status,
        GameStatus::Active
    );
}
