//! Ephemeral-Rollups move-path suite.
//!
//! Exercises `record_move` — the instruction that runs on the ER while a game
//! is delegated — across its happy path and every guard: replay nonce, causal
//! parent-nonce, turn enforcement, move legality, session expiry, and on-chain
//! game-end detection (checkmate). Runs in-process; no live ER required.
//!
//! Prereq: build the program first — `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use common::*;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{GameResult, GameStatus};

const GAME_ID: u64 = 42;

#[tokio::test]
async fn records_a_legal_move_and_advances_state() {
    let session = Keypair::new();
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let board = start_board();

    let mut ctx = start(vec![
        game_account(GAME_ID, white, black, board, 1, 0, GameStatus::Active),
        session_account(GAME_ID, white, session.pubkey(), i64::MAX, true),
    ])
    .await;

    let mv = uci("e2e4");
    let next = apply(&board, &mv);
    let ix = record_move_ix(GAME_ID, &white, &session.pubkey(), mv, next, 1, Some(0));

    send(&mut ctx, ix, &[&session]).await.expect("legal move should succeed");

    let g = fetch_game(&mut ctx, GAME_ID).await;
    assert_eq!(g.nonce, 1, "nonce advances");
    assert_eq!(g.turn, 2, "turn flips to black");
    assert_eq!(g.move_count, 1);
    assert_eq!(g.board_state, next, "board matches applied move");
    assert_eq!(g.status, GameStatus::Active);
}

#[tokio::test]
async fn rejects_replayed_nonce() {
    let session = Keypair::new();
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let board = start_board();

    let mut ctx = start(vec![
        game_account(GAME_ID, white, black, board, 1, 0, GameStatus::Active),
        session_account(GAME_ID, white, session.pubkey(), i64::MAX, true),
    ])
    .await;

    // game.nonce == 0 requires nonce == 1; sending 3 must be rejected.
    let mv = uci("e2e4");
    let next = apply(&board, &mv);
    let ix = record_move_ix(GAME_ID, &white, &session.pubkey(), mv, next, 3, None);

    let err = send(&mut ctx, ix, &[&session]).await.unwrap_err();
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::InvalidNonce)));
}

#[tokio::test]
async fn rejects_parent_nonce_mismatch() {
    let session = Keypair::new();
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let board = start_board();

    let mut ctx = start(vec![
        game_account(GAME_ID, white, black, board, 1, 0, GameStatus::Active),
        session_account(GAME_ID, white, session.pubkey(), i64::MAX, true),
    ])
    .await;

    let mv = uci("e2e4");
    let next = apply(&board, &mv);
    // game.nonce == 0, but the client claims a parent of 5 → causal break.
    let ix = record_move_ix(GAME_ID, &white, &session.pubkey(), mv, next, 1, Some(5));

    let err = send(&mut ctx, ix, &[&session]).await.unwrap_err();
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::ParentNonceMismatch)));
}

#[tokio::test]
async fn rejects_move_out_of_turn() {
    let session = Keypair::new();
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let board = start_board();

    // turn == 1 (white to move), but the session belongs to black.
    let mut ctx = start(vec![
        game_account(GAME_ID, white, black, board, 1, 0, GameStatus::Active),
        session_account(GAME_ID, black, session.pubkey(), i64::MAX, true),
    ])
    .await;

    let ix = record_move_ix(GAME_ID, &black, &session.pubkey(), uci("e2e4"), board, 1, Some(0));

    let err = send(&mut ctx, ix, &[&session]).await.unwrap_err();
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::NotYourTurn)));
}

#[tokio::test]
async fn rejects_illegal_move() {
    let session = Keypair::new();
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let board = start_board();

    let mut ctx = start(vec![
        game_account(GAME_ID, white, black, board, 1, 0, GameStatus::Active),
        session_account(GAME_ID, white, session.pubkey(), i64::MAX, true),
    ])
    .await;

    // e2e5 is not a legal pawn move from the start position.
    let ix = record_move_ix(GAME_ID, &white, &session.pubkey(), uci("e2e5"), board, 1, Some(0));

    let err = send(&mut ctx, ix, &[&session]).await.unwrap_err();
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::InvalidMove)));
}

#[tokio::test]
async fn rejects_expired_session() {
    let session = Keypair::new();
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let board = start_board();

    // expires_at = -1 is strictly before any non-negative clock timestamp.
    let mut ctx = start(vec![
        game_account(GAME_ID, white, black, board, 1, 0, GameStatus::Active),
        session_account(GAME_ID, white, session.pubkey(), -1, true),
    ])
    .await;

    let mv = uci("e2e4");
    let next = apply(&board, &mv);
    let ix = record_move_ix(GAME_ID, &white, &session.pubkey(), mv, next, 1, Some(0));

    let err = send(&mut ctx, ix, &[&session]).await.unwrap_err();
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::SessionExpired)));
}

#[tokio::test]
async fn detects_checkmate_and_finishes_game() {
    let session = Keypair::new();
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    // Position after 1. f3 e5 2. g4 — black to move, Qd8-h4 is Fool's Mate.
    let board = board_from_fen("rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2");

    // turn == 4 (even → black to move); nonce == 3 so the mating move is #4.
    let mut ctx = start(vec![
        game_account(GAME_ID, white, black, board, 4, 3, GameStatus::Active),
        session_account(GAME_ID, black, session.pubkey(), i64::MAX, true),
    ])
    .await;

    let mv = uci("d8h4");
    let next = apply(&board, &mv);
    let ix = record_move_ix(GAME_ID, &black, &session.pubkey(), mv, next, 4, Some(3));

    send(&mut ctx, ix, &[&session]).await.expect("mating move is legal");

    let g = fetch_game(&mut ctx, GAME_ID).await;
    assert_eq!(g.status, GameStatus::Finished, "checkmate ends the game");
    assert!(
        matches!(g.result, GameResult::Winner(w) if w == black),
        "black (the mating side) should be recorded as the winner"
    );
}
