//! Tests for the ER-unavailability forced-recovery path added in
//! `delegation_ix::force_recovery` / `governance_ix::recover_stuck_delegation`.
//!
//! `request_force_undelegate` / `force_undelegate_after_timeout` CPI into the
//! real MagicBlock delegation program, so their happy paths need a live ER
//! and aren't covered here — see docs/runbooks/magicblock-lifecycle-devnet.md.
//! What's covered in-process: the account-validation guards that reject
//! before any CPI runs (mirroring `er_delegation_tests.rs`'s existing
//! pattern), and the full `recover_stuck_delegation` payout path, which is
//! pure program logic with no external CPI.
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use anchor_lang::{AnchorDeserialize, Discriminator, InstructionData, ToAccountMetas};
use common::*;
use solana_sdk::{
    account::Account,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::events::StuckDelegationRecovered;
use xfchess_game::state::GameStatus;

const GAME_ID: u64 = 42;

fn anchor_constraint_address() -> u32 {
    anchor_lang::error::ErrorCode::ConstraintAddress as u32
}

fn escrow_pda(game_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"escrow", &game_id.to_le_bytes()], &xfchess_game::ID)
}

fn treasury_vault_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"treasury_vault"], &xfchess_game::ID)
}

/// A `Game` PDA wiped to zero bytes, as `force_undelegate_after_timeout`
/// leaves it — owned by the program, holding only its zero-data
/// rent-exempt minimum (`Rent::minimum_balance(0)` on mainnet/devnet).
fn wiped_game_account() -> Account {
    Account {
        lamports: 890_880,
        data: vec![],
        owner: xfchess_game::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn recover_stuck_delegation_ix(
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    dispute_authority: Pubkey,
) -> Instruction {
    let accounts =
        xfchess_game::__client_accounts_recover_stuck_delegation::RecoverStuckDelegation {
            game: game_pda(game_id).0,
            escrow_pda: escrow_pda(game_id).0,
            treasury_vault: treasury_vault_pda().0,
            white_authority: white,
            black_authority: black,
            dispute_authority,
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None);
    let data = xfchess_game::instruction::RecoverStuckDelegation { game_id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

fn request_force_undelegate_ix(
    game_id: u64,
    payer: Pubkey,
    owner_program: Pubkey,
    delegation_program: Pubkey,
) -> Instruction {
    let accounts =
        xfchess_game::__client_accounts_request_force_undelegate_ctx::RequestForceUndelegateCtx {
            game: game_pda(game_id).0,
            payer,
            owner_program,
            undelegation_request_pda: Pubkey::new_unique(),
            delegation_record_pda: Pubkey::new_unique(),
            delegation_metadata_pda: Pubkey::new_unique(),
            delegation_program,
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None);
    let data = xfchess_game::instruction::RequestForceUndelegate { game_id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

fn force_undelegate_after_timeout_ix(
    game_id: u64,
    owner_program: Pubkey,
    delegation_program: Pubkey,
) -> Instruction {
    let accounts =
        xfchess_game::__client_accounts_force_undelegate_after_timeout_ctx::ForceUndelegateAfterTimeoutCtx {
            game: game_pda(game_id).0,
            owner_program,
            undelegation_request_pda: Pubkey::new_unique(),
            delegation_record_pda: Pubkey::new_unique(),
            delegation_metadata_pda: Pubkey::new_unique(),
            delegation_rent_payer: Pubkey::new_unique(),
            commit_state_pda: Pubkey::new_unique(),
            commit_record_pda: Pubkey::new_unique(),
            commit_reimbursement: Pubkey::new_unique(),
            delegation_program,
        }
        .to_account_metas(None);
    let data = xfchess_game::instruction::ForceUndelegateAfterTimeout { game_id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

/// Load the real dispute authority keypair from the gitignored keyfile.
/// Returns None (test skips the signing path) when it isn't present (e.g. CI).
fn dispute_authority_keypair() -> Option<Keypair> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../keys/dispute_authority.json"
    );
    read_keypair_file(path).ok()
}

/// Decode an anchor `emit!`ed event of type `T` out of a transaction's
/// program logs (format: `Program data: <base64(discriminator || borsh)>`).
/// Returns `None` if no log line decodes to `T`'s discriminator.
fn find_event<T: AnchorDeserialize + Discriminator>(logs: &[String]) -> Option<T> {
    use base64::Engine;
    for log in logs {
        let Some(b64) = log.strip_prefix("Program data: ") else {
            continue;
        };
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) else {
            continue;
        };
        if bytes.len() < 8 || &bytes[..8] != T::DISCRIMINATOR {
            continue;
        }
        if let Ok(event) = T::try_from_slice(&bytes[8..]) {
            return Some(event);
        }
    }
    None
}

// ── recover_stuck_delegation ────────────────────────────────────────────────

// docs/PRE_MAINNET_E2E_PLAN.md §6.1: these three tests need the real
// dispute_authority signer (`keys/dispute_authority.json`, gitignored, not
// present on a fresh clone or CI runner) — `#[ignore]` makes that show up as
// an explicit skip in `cargo test`'s summary ("N ignored") instead of
// silently folding into "N passed" via an easy-to-miss `eprintln!`. Run with
// `cargo test -- --ignored` once the keyfile is provisioned locally/in CI.

#[tokio::test]
#[ignore = "needs keys/dispute_authority.json (devnet-only, gitignored) — see docs/PRE_MAINNET_E2E_PLAN.md §1.5/§6.1"]
async fn recover_stuck_delegation_splits_escrow_and_sweeps_game_rent() {
    let Some(authority) = dispute_authority_keypair() else {
        eprintln!("skip: keys/dispute_authority.json not present");
        return;
    };
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![
        (game_pda(GAME_ID).0, wiped_game_account()),
        (escrow_pda(GAME_ID).0, system_account(2_000_000_000)),
        (treasury_vault_pda().0, system_account(0)),
        (white, system_account(0)),
        (black, system_account(0)),
    ])
    .await;

    let ix = recover_stuck_delegation_ix(GAME_ID, white, black, authority.pubkey());
    send(&mut ctx, ix, &[&authority]).await.unwrap();

    let white_bal = ctx
        .banks_client
        .get_account(white)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let black_bal = ctx
        .banks_client
        .get_account(black)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let treasury_bal = ctx
        .banks_client
        .get_account(treasury_vault_pda().0)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let game_after = ctx
        .banks_client
        .get_account(game_pda(GAME_ID).0)
        .await
        .unwrap();

    assert_eq!(white_bal, 1_000_000_000, "white must get half the escrow");
    assert_eq!(black_bal, 1_000_000_000, "black must get half the escrow");
    assert_eq!(
        treasury_bal, 890_880,
        "the wiped game PDA's own rent must sweep to the treasury"
    );
    assert_eq!(
        game_after.map(|a| a.lamports).unwrap_or(0),
        0,
        "the game PDA must be fully drained after recovery"
    );
}

/// docs/PRE_MAINNET_E2E_PLAN.md §1.5 gap 1: before the `StuckDelegationRecovered`
/// event was added, there was no on-chain audit trail at all for this
/// instruction — grepping the handler for `emit!` returned nothing. This
/// asserts the event actually fires, with the correct game/authority/split
/// fields, on a successful recovery.
#[tokio::test]
#[ignore = "needs keys/dispute_authority.json (devnet-only, gitignored) — see docs/PRE_MAINNET_E2E_PLAN.md §1.5/§6.1"]
async fn recover_stuck_delegation_emits_audit_event() {
    let Some(authority) = dispute_authority_keypair() else {
        eprintln!("skip: keys/dispute_authority.json not present");
        return;
    };
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![
        (game_pda(GAME_ID).0, wiped_game_account()),
        (escrow_pda(GAME_ID).0, system_account(2_000_000_000)),
        (treasury_vault_pda().0, system_account(0)),
        (white, system_account(0)),
        (black, system_account(0)),
    ])
    .await;

    let ix = recover_stuck_delegation_ix(GAME_ID, white, black, authority.pubkey());
    let blockhash = ctx.last_blockhash;
    let payer_pk = ctx.payer.pubkey();
    let mut tx = solana_sdk::transaction::Transaction::new_with_payer(&[ix], Some(&payer_pk));
    tx.sign(&[&ctx.payer, &authority], blockhash);
    let result = ctx
        .banks_client
        .process_transaction_with_metadata(tx)
        .await
        .expect("transport-level failure");
    result
        .result
        .expect("recover_stuck_delegation should succeed");

    let logs = result
        .metadata
        .expect("metadata must be present")
        .log_messages;
    let event: StuckDelegationRecovered =
        find_event(&logs).expect("StuckDelegationRecovered event must be emitted");

    assert_eq!(event.game_id, GAME_ID);
    assert_eq!(event.dispute_authority, authority.pubkey());
    assert_eq!(event.white_authority, white);
    assert_eq!(event.black_authority, black);
    assert_eq!(event.white_share, 1_000_000_000);
    assert_eq!(event.black_share, 1_000_000_000);
}

#[tokio::test]
#[ignore = "needs keys/dispute_authority.json (devnet-only, gitignored) — see docs/PRE_MAINNET_E2E_PLAN.md §1.5/§6.1"]
async fn recover_stuck_delegation_rejects_non_stuck_game() {
    let Some(authority) = dispute_authority_keypair() else {
        eprintln!("skip: keys/dispute_authority.json not present");
        return;
    };
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![
        game_account(
            GAME_ID,
            white,
            black,
            start_board(),
            1,
            0,
            GameStatus::Active,
        ),
        (escrow_pda(GAME_ID).0, system_account(2_000_000_000)),
        (treasury_vault_pda().0, system_account(0)),
        (white, system_account(0)),
        (black, system_account(0)),
    ])
    .await;

    let ix = recover_stuck_delegation_ix(GAME_ID, white, black, authority.pubkey());
    let err = send(&mut ctx, ix, &[&authority]).await.unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::GameNotStuckDelegation)),
        "a live, populated Game account must not be treated as force-recovered"
    );
}

#[tokio::test]
async fn recover_stuck_delegation_rejects_wrong_authority() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let impostor = Keypair::new();

    let mut ctx = start(vec![
        (game_pda(GAME_ID).0, wiped_game_account()),
        (escrow_pda(GAME_ID).0, system_account(2_000_000_000)),
        (treasury_vault_pda().0, system_account(0)),
        (white, system_account(0)),
        (black, system_account(0)),
    ])
    .await;

    let ix = recover_stuck_delegation_ix(GAME_ID, white, black, impostor.pubkey());
    let err = send(&mut ctx, ix, &[&impostor]).await.unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::UnauthorizedDisputeResolution)),
        "only the platform dispute authority may trigger recovery"
    );
}

// ── request_force_undelegate / force_undelegate_after_timeout guards ──────
//
// Both instructions CPI into the real MagicBlock delegation program, so
// their happy paths need a live ER. What's covered here mirrors
// `er_delegation_tests.rs`'s existing `undelegate_rejects_spoofed_*` tests:
// the `address =` constraints on `owner_program` / `delegation_program`
// reject before any CPI runs, independent of what the other (delegation-
// program-owned) accounts contain.

#[tokio::test]
async fn request_force_undelegate_rejects_spoofed_owner_program() {
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
    let ix = request_force_undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::new_unique(), // spoofed owner_program
        ephemeral_rollups_sdk::id(),
    );
    let err = send(&mut ctx, ix, &[]).await.unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(6000 + xfchess_game::errors::GameErrorCode::InvalidOwnerProgram as u32),
        "wrong owner_program must be rejected before the CPI"
    );
}

#[tokio::test]
async fn request_force_undelegate_rejects_spoofed_delegation_program() {
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
    let ix = request_force_undelegate_ix(
        GAME_ID,
        payer,
        xfchess_game::ID,
        Pubkey::new_unique(), // spoofed delegation_program
    );
    let err = send(&mut ctx, ix, &[]).await.unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(anchor_constraint_address()),
        "wrong delegation_program must be rejected by Anchor's address constraint"
    );
}

#[tokio::test]
async fn force_undelegate_after_timeout_rejects_spoofed_owner_program() {
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

    let ix = force_undelegate_after_timeout_ix(
        GAME_ID,
        Pubkey::new_unique(), // spoofed owner_program
        ephemeral_rollups_sdk::id(),
    );
    let err = send(&mut ctx, ix, &[]).await.unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(6000 + xfchess_game::errors::GameErrorCode::InvalidOwnerProgram as u32),
        "wrong owner_program must be rejected before the CPI"
    );
}
