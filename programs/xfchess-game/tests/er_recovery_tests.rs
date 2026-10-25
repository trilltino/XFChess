#[path = "common/agave.rs"]
pub mod agave;
use anyhow::Context as _;
pub mod common;

use anchor_lang::{InstructionData, ToAccountMetas};
use common::*;
use solana_sdk::{
    account::Account,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::GameStatus;
use xfchess_game::state::{RecoveryIdentity, RecoveryStatus};

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

// Authorized signer fixtures and fail-closed recovery are tested without keys
// in account_security_tests; no private authority keyfile is needed.

#[tokio::test]
async fn recover_stuck_delegation_rejects_wrong_authority() -> anyhow::Result<()> {
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
    let err = send(&mut ctx, ix, &[&impostor])
        .await
        .err()
        .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::UnauthorizedDisputeResolution)),
        "only the platform dispute authority may trigger recovery"
    );
    Ok(())
}

fn recovery_identity_account(white: Pubkey, black: Pubkey) -> anyhow::Result<(Pubkey, Account)> {
    use anchor_lang::Space;
    let (key, bump) = Pubkey::find_program_address(
        &[RecoveryIdentity::SEED, &GAME_ID.to_le_bytes()],
        &xfchess_game::ID,
    );
    let record = RecoveryIdentity {
        game_id: GAME_ID,
        white,
        black,
        wager_amount: 1_000_000_000,
        status: RecoveryStatus::Delegated,
        bump,
    };
    Ok((
        key,
        program_account(&record, 8 + RecoveryIdentity::INIT_SPACE)?,
    ))
}

// BanksServer's metadata execution calls Bank::process_transaction_with_metadata
// without transaction signature verification (solana-banks-server 3.1.12).
// Deliberately inject the fixed authority's signer role, without accessing any
// authority secret. This tests SBF/CPI/account guards, NOT ed25519 authentication.
async fn execute_authority_fixture(
    ctx: &mut crate::agave::ProgramTestContext,
    ix: Instruction,
) -> anyhow::Result<solana_sdk::transaction::TransactionError> {
    let mut tx =
        solana_sdk::transaction::Transaction::new_with_payer(&[ix], Some(&ctx.payer.pubkey()));
    tx.partial_sign(&[&ctx.payer], ctx.last_blockhash);
    let result = ctx
        .banks_client
        .process_transaction_with_metadata(tx)
        .await?;
    result
        .result
        .err()
        .context("expected the recovery guard to reject this fixture")
}

#[tokio::test]
async fn recovery_v2_authority_cannot_substitute_either_player() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let impostor = Pubkey::new_unique();
    let record = recovery_identity_account(white, black)?;
    let mut ctx = start(vec![
        (game_pda(GAME_ID).0, wiped_game_account()),
        (escrow_pda(GAME_ID).0, system_account(2_001_000_000)),
        (treasury_vault_pda().0, system_account(1_000_000)),
        (white, system_account(1_000_000)),
        (black, system_account(1_000_000)),
        (impostor, system_account(1_000_000)),
        (
            xfchess_game::constants::dispute_authority::ID,
            system_account(1_000_000),
        ),
        record.clone(),
    ])
    .await;
    let keys = [
        game_pda(GAME_ID).0,
        escrow_pda(GAME_ID).0,
        treasury_vault_pda().0,
        white,
        black,
        impostor,
        record.0,
    ];
    let mut before = Vec::new();
    for key in keys {
        before.push(ctx.banks_client.get_account(key).await?);
    }
    for (white_recipient, black_recipient) in [(impostor, black), (white, impostor)] {
        let mut ix = recover_stuck_delegation_ix(
            GAME_ID,
            white_recipient,
            black_recipient,
            xfchess_game::constants::dispute_authority::ID,
        );
        ix.data = xfchess_game::instruction::RecoverStuckDelegationV2 { game_id: GAME_ID }.data();
        ix.accounts
            .push(solana_sdk::instruction::AccountMeta::new(record.0, false));
        let err = execute_authority_fixture(&mut ctx, ix).await?;
        assert_eq!(
            custom_code(&err),
            Some(ec(GameErrorCode::RecoveryIdentityMismatch))
        );
        for (index, key) in keys.iter().enumerate() {
            assert_eq!(ctx.banks_client.get_account(*key).await?, before[index]);
        }
    }
    Ok(())
}

#[tokio::test]
async fn recovery_v2_pays_verified_wagers_and_preserves_extra_lamports() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let record = recovery_identity_account(white, black)?;
    let ctx = start(vec![
        (game_pda(GAME_ID).0, wiped_game_account()),
        (escrow_pda(GAME_ID).0, system_account(2_001_000_000)),
        (treasury_vault_pda().0, system_account(1_000_000)),
        (white, system_account(1_000_000)),
        (black, system_account(1_000_000)),
        (
            xfchess_game::constants::dispute_authority::ID,
            system_account(1_000_000),
        ),
        record.clone(),
    ])
    .await;
    let mut ix = recover_stuck_delegation_ix(
        GAME_ID,
        white,
        black,
        xfchess_game::constants::dispute_authority::ID,
    );
    ix.data = xfchess_game::instruction::RecoverStuckDelegationV2 { game_id: GAME_ID }.data();
    ix.accounts
        .push(solana_sdk::instruction::AccountMeta::new(record.0, false));
    let mut tx =
        solana_sdk::transaction::Transaction::new_with_payer(&[ix], Some(&ctx.payer.pubkey()));
    tx.partial_sign(&[&ctx.payer], ctx.last_blockhash);
    let result = ctx
        .banks_client
        .process_transaction_with_metadata(tx)
        .await?;
    result
        .result
        .context("verified SBF recovery should pay the original players")?;
    assert_eq!(ctx.banks_client.get_balance(white).await?, 1_001_000_000);
    assert_eq!(ctx.banks_client.get_balance(black).await?, 1_001_000_000);
    assert_eq!(
        ctx.banks_client.get_balance(escrow_pda(GAME_ID).0).await?,
        1_000_000
    );
    assert_eq!(
        ctx.banks_client.get_balance(treasury_vault_pda().0).await?,
        1_890_880
    );
    assert_eq!(ctx.banks_client.get_balance(game_pda(GAME_ID).0).await?, 0);
    let record_after = ctx
        .banks_client
        .get_account(record.0)
        .await?
        .context("permanent record must survive")?;
    use anchor_lang::AccountDeserialize;
    let state = RecoveryIdentity::try_deserialize(&mut record_after.data.as_slice())?;
    assert_eq!(state.status, RecoveryStatus::Recovered);
    assert_eq!(record_after.lamports, record.1.lamports);
    Ok(())
}

// Test owner_program and delegation_program address guards before CPI.
// Successful recovery requires the live delegation program.

#[tokio::test]
async fn request_force_undelegate_rejects_spoofed_owner_program() -> anyhow::Result<()> {
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
    let ix = request_force_undelegate_ix(
        GAME_ID,
        payer,
        Pubkey::new_unique(), // spoofed owner_program
        ephemeral_rollups_sdk::id(),
    );
    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(6000 + xfchess_game::errors::GameErrorCode::InvalidOwnerProgram as u32),
        "wrong owner_program must be rejected before the CPI"
    );
    Ok(())
}

#[tokio::test]
async fn request_force_undelegate_rejects_spoofed_delegation_program() -> anyhow::Result<()> {
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
    let ix = request_force_undelegate_ix(
        GAME_ID,
        payer,
        xfchess_game::ID,
        Pubkey::new_unique(), // spoofed delegation_program
    );
    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(anchor_constraint_address()),
        "wrong delegation_program must be rejected by Anchor's address constraint"
    );
    Ok(())
}

#[tokio::test]
async fn force_undelegate_after_timeout_rejects_spoofed_owner_program() -> anyhow::Result<()> {
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

    let ix = force_undelegate_after_timeout_ix(
        GAME_ID,
        Pubkey::new_unique(), // spoofed owner_program
        ephemeral_rollups_sdk::id(),
    );
    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(6000 + xfchess_game::errors::GameErrorCode::InvalidOwnerProgram as u32),
        "wrong owner_program must be rejected before the CPI"
    );
    Ok(())
}

fn live_game_account(
    white: Pubkey,
    black: Pubkey,
    updated_at: i64,
) -> anyhow::Result<solana_sdk::account::Account> {
    Ok({
        use anchor_lang::Space;
        use xfchess_game::state::{Game, GameResult, GameType, MatchType};
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
            move_count: 10,
            halfmove_clock: 0,
            turn: 11,
            created_at: updated_at,
            updated_at,
            wager_amount: 0,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Free,
            country_fee: 0,
            base_time_seconds: 300,
            increment_seconds: 0,
            bump: game_pda(GAME_ID).1,
            is_delegated: true,
            tournament_id: None,
            nonce: 10,
            draw_offered_by: None,
        };
        program_account(&game, 8 + Game::INIT_SPACE)?
    })
}

async fn request_on_game_idle_for(
    idle_secs: i64,
) -> anyhow::Result<solana_sdk::transaction::TransactionError> {
    Ok({
        use solana_sdk::{account::AccountSharedData, clock::Clock};
        let mut ctx = start(vec![]).await;
        let now = ctx
            .banks_client
            .get_sysvar::<Clock>()
            .await
            .context("required operation value")?
            .unix_timestamp;
        let account =
            live_game_account(Pubkey::new_unique(), Pubkey::new_unique(), now - idle_secs)?;
        ctx.set_account(&game_pda(GAME_ID).0, &AccountSharedData::from(account));

        let payer = ctx.payer.pubkey();
        let ix = request_force_undelegate_ix(
            GAME_ID,
            payer,
            xfchess_game::ID,
            ephemeral_rollups_sdk::id(),
        );
        send(&mut ctx, ix, &[])
            .await
            .err()
            .context("required operation value")?
    })
}

#[tokio::test]
async fn request_force_undelegate_rejects_a_live_game() -> anyhow::Result<()> {
    let err = request_on_game_idle_for(30).await?;
    assert_eq!(
        custom_code(&err),
        Some(6000 + xfchess_game::errors::GameErrorCode::GameNotStuckDelegation as u32),
        "a game the ER is still serving must not be pushed toward a wipe"
    );
    Ok(())
}

#[tokio::test]
async fn request_force_undelegate_allows_a_game_idle_past_its_timeout() -> anyhow::Result<()> {
    // Past the 300s clock: the guard passes and only the (unloaded)
    // delegation program CPI fails.
    let err = request_on_game_idle_for(301 + 60).await?;
    assert_ne!(
        custom_code(&err),
        Some(6000 + xfchess_game::errors::GameErrorCode::GameNotStuckDelegation as u32),
    );
    Ok(())
}
