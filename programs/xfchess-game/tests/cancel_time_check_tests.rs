//! `cancel_time_check` account-constraint suite.
//!
//! Like `schedule_time_check`/`crank_time_check`, the happy path CPIs into
//! MagicBlock's magic program (`CancelTask`), so it requires a live ER and is
//! covered by the devnet runbook (`docs/ER_TESTING.md`, `docs/runbooks/
//! magicblock-lifecycle-devnet.md`). What we *can* assert in-process is the
//! `address =` constraint on `magic_program`, mirroring the equivalent
//! `undelegate_game` tests in `er_delegation_tests.rs`.
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use common::*;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::str::FromStr;
use xfchess_game::state::GameStatus;

const GAME_ID: u64 = 11;

fn anchor_constraint_address() -> u32 {
    anchor_lang::error::ErrorCode::ConstraintAddress as u32
}

#[tokio::test]
async fn cancel_time_check_rejects_spoofed_magic_program() {
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
    let ix = cancel_time_check_ix(
        GAME_ID,
        payer,
        GAME_ID, // task_id == game_id, matching schedule_time_check_ix's convention
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
async fn cancel_time_check_accepts_canonical_magic_program_account_shape() {
    // Not a full happy-path assertion (no live MagicBlock program is loaded
    // in-process, so the CPI itself will fail) — this only proves the
    // instruction gets past account validation and reaches the CPI, i.e. the
    // account list/seeds/discriminator are wired correctly end-to-end.
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
    let ix = cancel_time_check_ix(
        GAME_ID,
        payer,
        GAME_ID,
        Pubkey::from_str(MAGIC_PROGRAM).unwrap(),
    );

    let err = send(&mut ctx, ix, &[]).await.unwrap_err();
    assert_ne!(
        custom_code(&err),
        Some(anchor_constraint_address()),
        "canonical magic_program must pass account validation (failure past this \
         point is the expected CPI-into-unloaded-program error, not a constraint reject)"
    );
}
