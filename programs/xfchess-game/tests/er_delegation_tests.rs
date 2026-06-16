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
use solana_sdk::signature::Signer;
use std::str::FromStr;
use xfchess_game::state::GameStatus;

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
        Pubkey::new_unique(),                  // spoofed magic_context
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
