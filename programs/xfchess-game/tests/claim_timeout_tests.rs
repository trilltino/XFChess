//! Integration-level coverage for `ClaimTimeout` (docs/PRE_MAINNET_E2E_PLAN.md
//! §1.3). `lifecycle::terminal::finish_by_timeout`'s pure-function branches
//! already have unit coverage in `terminal.rs`, but nothing previously drove
//! the *instruction* itself through the real `Clock` sysvar — this exercises
//! the full `ClaimTimeout` dispatch (permissionless caller, on-chain
//! `Clock::get()`, PDA lookup) for the two branches the existing
//! `er_delegation_tests.rs` test doesn't cover: the `TimeoutNotExpired`
//! rejection, and the mirror parity (black timed out -> white wins).
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use anchor_lang::Space;
use common::*;
use solana_sdk::{account::AccountSharedData, clock::Clock, pubkey::Pubkey, signer::Signer};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{Game, GameResult, GameStatus, GameType, MatchType};

const GAME_ID: u64 = 90_001;
// `base_time_seconds` only needs to be nonzero to select the "timed game"
// branch of `lifecycle::clock::inactivity_window_seconds` — the actual
// window is now a fixed `TIMED_GAME_INACTIVITY_WINDOW_SECONDS` (90s),
// independent of this value. See that function's doc comment for why it's
// no longer proportional to base time.
const BASE_TIME_SECONDS: u64 = 300;

/// A timed, active game with caller-controlled `updated_at`/`turn`, so the
/// test can place it on either side of the inactivity window.
fn timed_active_game(
    white: Pubkey,
    black: Pubkey,
    turn: u16,
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
        move_count: turn.saturating_sub(1),
        halfmove_clock: 0,
        turn,
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
        is_delegated: false,
        tournament_id: None,
        nonce: 0,
        draw_offered_by: None,
    };
    (pda, program_account(&game, 8 + Game::INIT_SPACE))
}

#[tokio::test]
async fn claim_timeout_rejects_before_inactivity_window_elapses() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![]).await;
    let now = ctx
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .unwrap()
        .unix_timestamp;

    // Only 30s elapsed since the last move; the fixed timed-game inactivity
    // window (`TIMED_GAME_INACTIVITY_WINDOW_SECONDS`) is 90s.
    let (game_key, game_data) = timed_active_game(white, black, 1, now - 30);
    ctx.set_account(&game_key, &AccountSharedData::from(game_data));

    let ix = claim_timeout_ix(GAME_ID, ctx.payer.pubkey());
    let err = send(&mut ctx, ix, &[])
        .await
        .expect_err("claim_timeout must reject before the inactivity window elapses");

    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::TimeoutNotExpired)),
        "expected TimeoutNotExpired, got {err:?}"
    );

    let game = fetch_game(&mut ctx, GAME_ID).await;
    assert_eq!(
        game.status,
        GameStatus::Active,
        "a rejected timeout claim must not mutate the game"
    );
}

#[tokio::test]
async fn claim_timeout_awards_white_when_black_flags() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![]).await;
    let now = ctx
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .unwrap()
        .unix_timestamp;

    // turn = 2 (even) -> black's clock is the one that expired, per
    // `finish_by_timeout`'s `white_timed_out = turn % 2 == 1` branch. The
    // sibling test (`er_delegation_tests::claim_timeout_mutates_only_game_even_when_delegated`)
    // only covers the odd-turn (white-timed-out) parity.
    // 200s elapsed, comfortably past the fixed 90s window.
    let (game_key, game_data) = timed_active_game(white, black, 2, now - 200);
    ctx.set_account(&game_key, &AccountSharedData::from(game_data));

    let ix = claim_timeout_ix(GAME_ID, ctx.payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .expect("claim_timeout should succeed once the inactivity window has elapsed");

    let game = fetch_game(&mut ctx, GAME_ID).await;
    assert_eq!(game.status, GameStatus::Finished);
    assert_eq!(
        game.result,
        GameResult::Winner(white),
        "black's clock expired (even turn) -> white must win"
    );
}

#[tokio::test]
async fn claim_timeout_cancels_zero_move_game_instead_of_awarding_black() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![]).await;
    let now = ctx
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .unwrap()
        .unix_timestamp;

    let (game_key, game_data) = timed_active_game(white, black, 1, now - 200);
    ctx.set_account(&game_key, &AccountSharedData::from(game_data));

    let ix = claim_timeout_ix(GAME_ID, ctx.payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .expect("claim_timeout should cancel an expired zero-move game");

    let game = fetch_game(&mut ctx, GAME_ID).await;
    assert_eq!(game.status, GameStatus::Cancelled);
    assert_eq!(
        game.result,
        GameResult::None,
        "zero moves means no winner and no one-sided payout"
    );
}
