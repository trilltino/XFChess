//! Delegation / undelegation account-constraint suite.
//!
//! The happy-path delegate/undelegate flows CPI into the MagicBlock delegation
//! and magic programs, so they require a live ER and are covered by the devnet
//! runbook in `docs/ER_TESTING.md`. What we *can* assert in-process are the
//! account-validation guards — in particular the `address =` constraints on the
//! magic accounts added to `undelegate_game` (a hardening fix). These reject
//! before any CPI, so no external program is needed.
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use common::*;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::str::FromStr;
use xfchess_game::state::{GameResult, GameStatus};

const GAME_ID: u64 = 7;

fn anchor_constraint_address() -> u32 {
    anchor_lang::error::ErrorCode::ConstraintAddress as u32
}

#[tokio::test]
async fn undelegate_rejects_spoofed_magic_context() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![game_account(
        GAME_ID,
        white,
        black,
        start_board(),
        1,
        0,
        GameStatus::Active,
    )])
    .await;

    let payer = ctx.payer.pubkey();
    let ix = undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::new_unique(), // spoofed magic_context
        Pubkey::from_str(MAGIC_PROGRAM).unwrap(),
    );

    let err = send(&mut ctx, ix, &[]).await.unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(anchor_constraint_address()),
        "wrong magic_context must be rejected by the address constraint"
    );
}

#[tokio::test]
async fn undelegate_rejects_spoofed_magic_program() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![game_account(
        GAME_ID,
        white,
        black,
        start_board(),
        1,
        0,
        GameStatus::Active,
    )])
    .await;

    let payer = ctx.payer.pubkey();
    let ix = undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::from_str(MAGIC_CONTEXT).unwrap(),
        Pubkey::new_unique(), // spoofed magic_program
    );

    let err = send(&mut ctx, ix, &[]).await.unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(anchor_constraint_address()),
        "wrong magic_program must be rejected by the address constraint"
    );
}

#[tokio::test]
async fn undelegate_rejects_not_delegated_game_before_cpi() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![game_account_with_delegation(
        GAME_ID,
        white,
        black,
        start_board(),
        1,
        0,
        GameStatus::Active,
        false,
    )])
    .await;

    let payer = ctx.payer.pubkey();
    let ix = undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::from_str(MAGIC_CONTEXT).unwrap(),
        Pubkey::from_str(MAGIC_PROGRAM).unwrap(),
    );

    let err = send(&mut ctx, ix, &[]).await.unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::GameNotDelegated)),
        "non-delegated games must reject before MagicBlock CPI"
    );
}

/// MagicBlock advisory (2026-07-22): `ephemeral_rollups_sdk::cpi::undelegate_account`
/// (SDK <= 0.16.1) checked `buffer` was a signer owned by the delegation program,
/// but never that it was *this account's own* buffer — any delegation-owned signer
/// buffer was accepted, letting an attacker substitute a manufactured buffer from
/// their own delegated account to overwrite someone else's restored account data.
/// We replicate the fix (see `magicblock::delegation::undelegate_buffer_pda`) since
/// bumping the SDK to 0.16.2 isn't possible yet (dependency ceiling — see
/// docs/MAGICBLOCK_INTEGRATION.md). This asserts a non-canonical buffer is rejected
/// before the CPI runs, no live ER or external program required.
#[tokio::test]
async fn process_undelegation_rejects_non_canonical_buffer() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![game_account(
        GAME_ID,
        white,
        black,
        start_board(),
        1,
        0,
        GameStatus::Active,
    )])
    .await;

    let payer = ctx.payer.pubkey();
    let spoofed_buffer = Pubkey::new_unique(); // not the canonical undelegate-buffer PDA
    let ix = process_undelegation_ix(
        GAME_ID,
        payer,
        spoofed_buffer,
        vec![b"game".to_vec(), GAME_ID.to_le_bytes().to_vec()],
    );

    let err = send(&mut ctx, ix, &[]).await.unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::InvalidUndelegationBuffer)),
        "a buffer that isn't this account's canonical undelegate-buffer PDA must be rejected"
    );
}

#[tokio::test]
async fn resign_mutates_only_game_even_when_delegated() {
    let white = Keypair::new();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![
        game_account(
            GAME_ID,
            white.pubkey(),
            black,
            start_board(),
            1,
            0,
            GameStatus::Active,
        ),
        (white.pubkey(), system_account(1_000_000)),
    ])
    .await;

    let ix = resign_ix(GAME_ID, white.pubkey());
    send(&mut ctx, ix, &[&white]).await.unwrap();

    let game = fetch_game(&mut ctx, GAME_ID).await;
    assert_eq!(game.status, GameStatus::Finished);
    assert_eq!(game.result, GameResult::Winner(black));
    assert!(
        game.is_delegated,
        "terminal ER transition must not undelegate"
    );
}

#[tokio::test]
async fn claim_timeout_mutates_only_game_even_when_delegated() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![game_account(
        GAME_ID,
        white,
        black,
        start_board(),
        1,
        0,
        GameStatus::Active,
    )])
    .await;

    let ix = claim_timeout_ix(GAME_ID, ctx.payer.pubkey());
    send(&mut ctx, ix, &[]).await.unwrap();

    let game = fetch_game(&mut ctx, GAME_ID).await;
    assert_eq!(game.status, GameStatus::Finished);
    assert_eq!(game.result, GameResult::Winner(black));
    assert!(
        game.is_delegated,
        "terminal ER transition must not undelegate"
    );
}
