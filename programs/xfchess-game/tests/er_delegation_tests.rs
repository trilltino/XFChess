#[path = "common/agave.rs"]
pub mod agave;
use anyhow::Context as _;
pub mod common;

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
async fn undelegate_rejects_spoofed_magic_context() -> anyhow::Result<()> {
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
    )?])
    .await;

    let payer = ctx.payer.pubkey();
    let ix = undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::new_unique(), // spoofed magic_context
        Pubkey::from_str(MAGIC_PROGRAM).context("required operation value")?,
    );

    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(anchor_constraint_address()),
        "wrong magic_context must be rejected by the address constraint"
    );
    Ok(())
}

#[tokio::test]
async fn undelegate_rejects_spoofed_magic_program() -> anyhow::Result<()> {
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
    )?])
    .await;

    let payer = ctx.payer.pubkey();
    let ix = undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::from_str(MAGIC_CONTEXT).context("required operation value")?,
        Pubkey::new_unique(), // spoofed magic_program
    );

    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(anchor_constraint_address()),
        "wrong magic_program must be rejected by the address constraint"
    );
    Ok(())
}

#[tokio::test]
async fn undelegate_rejects_not_delegated_game_before_cpi() -> anyhow::Result<()> {
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
    )?])
    .await;

    let payer = ctx.payer.pubkey();
    let ix = undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::from_str(MAGIC_CONTEXT).context("required operation value")?,
        Pubkey::from_str(MAGIC_PROGRAM).context("required operation value")?,
    );

    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::GameNotDelegated)),
        "non-delegated games must reject before MagicBlock CPI"
    );
    Ok(())
}

#[tokio::test]
async fn undelegate_rejects_an_outsider_on_a_live_game() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    // The fixture's fee payer is white, so the transaction payer is an outsider.
    let mut ctx = start(vec![game_account(
        GAME_ID,
        white,
        black,
        start_board(),
        1,
        0,
        GameStatus::Active,
    )?])
    .await;

    let payer = ctx.payer.pubkey();
    let ix = undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::from_str(MAGIC_CONTEXT).context("required operation value")?,
        Pubkey::from_str(MAGIC_PROGRAM).context("required operation value")?,
    );

    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::UnauthorizedAccess)),
        "an outsider must not pull a live game off the ER"
    );
    Ok(())
}

#[tokio::test]
async fn undelegate_lets_anyone_return_a_finished_game() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![game_account(
        GAME_ID,
        white,
        black,
        start_board(),
        1,
        0,
        GameStatus::Finished,
    )?])
    .await;

    let payer = ctx.payer.pubkey();
    let ix = undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::from_str(MAGIC_CONTEXT).context("required operation value")?,
        Pubkey::from_str(MAGIC_PROGRAM).context("required operation value")?,
    );

    // Without a MagicBlock validator the commit CPI fails; the point is that the
    // authority check passes.
    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("required operation value")?;
    assert_ne!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::UnauthorizedAccess)),
        "settlement must never be blocked on who undelegates a finished game"
    );
    Ok(())
}

#[tokio::test]
async fn process_undelegation_rejects_non_canonical_buffer() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    // To reach the SDK canonical-buffer check, the spoof must first be a signer
    // and be owned by the delegation program.
    let spoofed_buffer_kp = Keypair::new();
    let spoofed_buffer = spoofed_buffer_kp.pubkey();

    let mut ctx = start(vec![
        game_account(
            GAME_ID,
            white,
            black,
            start_board(),
            1,
            0,
            GameStatus::Active,
        )?,
        (
            spoofed_buffer,
            solana_sdk::account::Account {
                lamports: 1_000_000_000,
                data: vec![],
                owner: ephemeral_rollups_sdk::id(),
                executable: false,
                rent_epoch: 0,
            },
        ),
    ])
    .await;

    let payer = ctx.payer.pubkey();
    let mut ix = process_undelegation_ix(
        GAME_ID,
        payer,
        spoofed_buffer,
        vec![b"game".to_vec(), GAME_ID.to_le_bytes().to_vec()],
    );
    // Anchor does not mark bare AccountInfo as a signer. Set the buffer signer
    // flag manually; the Anchor canonical-PDA constraint now rejects before CPI.
    for meta in ix.accounts.iter_mut() {
        if meta.pubkey == spoofed_buffer {
            meta.is_signer = true;
        }
    }

    let err = send(&mut ctx, ix, &[&spoofed_buffer_kp])
        .await
        .err()
        .context("required operation value")?;
    assert_eq!(
        err,
        solana_sdk::transaction::TransactionError::InstructionError(
            0,
            solana_sdk::instruction::InstructionError::Custom(
                anchor_lang::error::ErrorCode::ConstraintSeeds as u32
            )
        ),
        "a noncanonical buffer must fail the Anchor seed constraint before the SDK CPI"
    );
    Ok(())
}

#[tokio::test]
async fn resign_mutates_only_game_even_when_delegated() -> anyhow::Result<()> {
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
        )?,
        (white.pubkey(), system_account(1_000_000)),
    ])
    .await;

    let ix = resign_ix(GAME_ID, white.pubkey());
    send(&mut ctx, ix, &[&white])
        .await
        .context("required operation value")?;

    let game = fetch_game(&mut ctx, GAME_ID).await?;
    assert_eq!(game.status, GameStatus::Finished);
    assert_eq!(game.result, GameResult::Winner(black));
    assert!(
        game.is_delegated,
        "terminal ER transition must not undelegate"
    );
    Ok(())
}

#[tokio::test]
async fn claim_timeout_mutates_only_game_even_when_delegated() -> anyhow::Result<()> {
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
    )?])
    .await;

    let ix = claim_timeout_ix(GAME_ID, ctx.payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .context("required operation value")?;

    let game = fetch_game(&mut ctx, GAME_ID).await?;
    assert_eq!(game.status, GameStatus::Cancelled);
    assert_eq!(
        game.result,
        GameResult::None,
        "zero moves means no winner and no one-sided payout"
    );
    assert!(
        game.is_delegated,
        "terminal ER transition must not undelegate"
    );
    Ok(())
}
