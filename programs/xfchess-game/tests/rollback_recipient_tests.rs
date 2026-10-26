//! S4: the rollback-after-timeout CPI must only refund the recipients recorded
//! by the Delegation Program. Needs the Delegation Program binary at
//! `target/deploy/dlp.so` (`solana program dump DELeGG... target/deploy/dlp.so -u devnet`).
#[path = "common/agave.rs"]
pub mod agave;
use anyhow::Context as _;
pub mod common;

use anchor_lang::{InstructionData, ToAccountMetas};
use common::*;
use dlp_api::compat::Pubkey as DlpKey;
use dlp_api::error::DlpError;
use dlp_api::state::{
    CommitRecord, DelegationMetadata, DelegationRecord, UndelegationRequest, UndelegationRequester,
};
use solana_sdk::{account::Account, instruction::Instruction, pubkey::Pubkey};

const GAME_ID: u64 = 42;
const LAMPORTS: u64 = 5_000_000;

fn dlp_key(key: &Pubkey) -> DlpKey {
    DlpKey::new_from_array(key.to_bytes())
}

fn owned_by_dlp(data: Vec<u8>) -> Account {
    Account {
        lamports: LAMPORTS,
        data,
        owner: ephemeral_rollups_sdk::id(),
        executable: false,
        rent_epoch: 0,
    }
}

struct Pdas {
    request: Pubkey,
    record: Pubkey,
    metadata: Pubkey,
    commit_state: Pubkey,
    commit_record: Pubkey,
}

/// PDA addresses come from the SDK builder the program itself uses.
fn pdas_for(game: &Pubkey) -> Pdas {
    let ix = ephemeral_rollups_sdk::instruction_builder::undelegate_with_rollback_after_timeout(
        *game,
        xfchess_game::ID,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );
    Pdas {
        request: ix.accounts[2].pubkey,
        record: ix.accounts[3].pubkey,
        metadata: ix.accounts[4].pubkey,
        commit_state: ix.accounts[6].pubkey,
        commit_record: ix.accounts[7].pubkey,
    }
}

fn request_data(game: &Pubkey, expires_at_slot: u64) -> anyhow::Result<Vec<u8>> {
    let mut bytes = vec![0u8; UndelegationRequest::size_with_discriminator()];
    UndelegationRequest {
        delegated_account: dlp_key(game),
        expires_at_slot,
    }
    .to_bytes_with_discriminator(&mut bytes)
    .map_err(|e| anyhow::anyhow!("request fixture: {e:?}"))?;
    Ok(bytes)
}

fn record_data() -> anyhow::Result<Vec<u8>> {
    let mut bytes = vec![0u8; DelegationRecord::size_with_discriminator()];
    DelegationRecord {
        authority: dlp_key(&Pubkey::new_unique()),
        owner: dlp_key(&xfchess_game::ID),
        delegation_slot: 0,
        lamports: LAMPORTS,
        commit_frequency_ms: 0,
    }
    .to_bytes_with_discriminator(&mut bytes)
    .map_err(|e| anyhow::anyhow!("record fixture: {e:?}"))?;
    Ok(bytes)
}

fn metadata_data(rent_payer: &Pubkey) -> anyhow::Result<Vec<u8>> {
    let mut bytes = vec![];
    DelegationMetadata {
        last_commit_id: 0,
        undelegation_requester: UndelegationRequester::from_allow_undelegation(false),
        seeds: vec![b"game".to_vec(), GAME_ID.to_le_bytes().to_vec()],
        rent_payer: dlp_key(rent_payer),
    }
    .to_bytes_with_discriminator(&mut bytes)
    .map_err(|e| anyhow::anyhow!("metadata fixture: {e:?}"))?;
    Ok(bytes)
}

fn commit_record_data(game: &Pubkey, identity: &Pubkey) -> anyhow::Result<Vec<u8>> {
    let mut bytes = vec![0u8; CommitRecord::size_with_discriminator()];
    CommitRecord {
        identity: dlp_key(identity),
        account: dlp_key(game),
        nonce: 1,
        lamports: LAMPORTS,
    }
    .to_bytes_with_discriminator(&mut bytes)
    .map_err(|e| anyhow::anyhow!("commit record fixture: {e:?}"))?;
    Ok(bytes)
}

struct Env {
    game: Pubkey,
    pdas: Pdas,
    rent_payer: Pubkey,
    accounts: Vec<(Pubkey, Account)>,
}

fn env(expires_at_slot: u64, pending_commit_identity: Option<Pubkey>) -> anyhow::Result<Env> {
    let game = game_pda(GAME_ID).0;
    let pdas = pdas_for(&game);
    let rent_payer = Pubkey::new_unique();
    let mut accounts = vec![
        (game, owned_by_dlp(vec![7u8; 64])),
        (
            pdas.request,
            owned_by_dlp(request_data(&game, expires_at_slot)?),
        ),
        (pdas.record, owned_by_dlp(record_data()?)),
        (pdas.metadata, owned_by_dlp(metadata_data(&rent_payer)?)),
        (rent_payer, system_account(1_000_000)),
    ];
    if let Some(identity) = pending_commit_identity {
        accounts.push((pdas.commit_state, owned_by_dlp(vec![1, 2, 3])));
        accounts.push((
            pdas.commit_record,
            owned_by_dlp(commit_record_data(&game, &identity)?),
        ));
        accounts.push((identity, system_account(1_000_000)));
    }
    Ok(Env {
        game,
        pdas,
        rent_payer,
        accounts,
    })
}

fn rollback_ix(env: &Env, rent_payer: Pubkey, reimbursement: Pubkey) -> Instruction {
    Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::__client_accounts_force_undelegate_after_timeout_ctx::ForceUndelegateAfterTimeoutCtx {
            game: env.game,
            owner_program: xfchess_game::ID,
            undelegation_request_pda: env.pdas.request,
            delegation_record_pda: env.pdas.record,
            delegation_metadata_pda: env.pdas.metadata,
            delegation_rent_payer: rent_payer,
            commit_state_pda: env.pdas.commit_state,
            commit_record_pda: env.pdas.commit_record,
            commit_reimbursement: reimbursement,
            delegation_program: ephemeral_rollups_sdk::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::ForceUndelegateAfterTimeout { game_id: GAME_ID }.data(),
    }
}

async fn snapshot(
    ctx: &mut agave::ProgramTestContext,
    keys: &[Pubkey],
) -> anyhow::Result<Vec<Option<Account>>> {
    let mut out = Vec::new();
    for key in keys {
        out.push(ctx.banks_client.get_account(*key).await?);
    }
    Ok(out)
}

async fn start_with_dlp(accounts: Vec<(Pubkey, Account)>) -> agave::ProgramTestContext {
    std::env::set_var(
        "SBF_OUT_DIR",
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy"),
    );
    let mut pt = agave::ProgramTest::new(PROGRAM, xfchess_game::ID, None);
    pt.add_program("dlp", ephemeral_rollups_sdk::id(), None);
    for (key, account) in accounts {
        pt.add_account(key, account);
    }
    pt.start_with_context().await
}

fn all_keys(env: &Env) -> Vec<Pubkey> {
    let mut keys: Vec<Pubkey> = env.accounts.iter().map(|(k, _)| *k).collect();
    keys.extend([env.pdas.commit_state, env.pdas.commit_record]);
    keys
}

#[tokio::test]
async fn rollback_refunds_the_recorded_rent_payer() -> anyhow::Result<()> {
    let env = env(0, None)?;
    let mut ctx = start_with_dlp(env.accounts.clone()).await;
    let before = ctx.banks_client.get_balance(env.rent_payer).await?;
    let ix = rollback_ix(&env, env.rent_payer, Pubkey::new_unique());
    send(&mut ctx, ix, &[]).await.context("valid rollback")?;
    let after = ctx.banks_client.get_balance(env.rent_payer).await?;
    assert_eq!(after, before + 3 * LAMPORTS, "request+record+metadata rent");
    let game = ctx
        .banks_client
        .get_account(env.game)
        .await?
        .context("game must remain")?;
    assert_eq!((game.owner, game.data.len()), (xfchess_game::ID, 0));
    Ok(())
}

#[tokio::test]
async fn rollback_rejects_a_substituted_rent_payer() -> anyhow::Result<()> {
    let mut env = env(0, None)?;
    let attacker = Pubkey::new_unique();
    env.accounts.push((attacker, system_account(1)));
    let mut ctx = start_with_dlp(env.accounts.clone()).await;
    let keys = all_keys(&env);
    let before = snapshot(&mut ctx, &keys).await?;
    let ix = rollback_ix(&env, attacker, Pubkey::new_unique());
    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("substituted rent payer must fail")?;
    assert_eq!(
        custom_code(&err),
        Some(DlpError::InvalidReimbursementAddressForDelegationRent as u32)
    );
    assert_eq!(snapshot(&mut ctx, &keys).await?, before);
    Ok(())
}

#[tokio::test]
async fn rollback_rejects_a_substituted_commit_reimbursement_and_pays_the_recorded_one(
) -> anyhow::Result<()> {
    let validator = Pubkey::new_unique();
    let attacker = Pubkey::new_unique();
    let mut env = env(0, Some(validator))?;
    env.accounts.push((attacker, system_account(1)));
    let mut ctx = start_with_dlp(env.accounts.clone()).await;
    let keys = all_keys(&env);
    let before = snapshot(&mut ctx, &keys).await?;
    let err = send(&mut ctx, rollback_ix(&env, env.rent_payer, attacker), &[])
        .await
        .err()
        .context("substituted reimbursement must fail")?;
    assert_eq!(
        custom_code(&err),
        Some(DlpError::InvalidPendingCommitState as u32)
    );
    assert_eq!(snapshot(&mut ctx, &keys).await?, before);

    let validator_before = ctx.banks_client.get_balance(validator).await?;
    send(&mut ctx, rollback_ix(&env, env.rent_payer, validator), &[])
        .await
        .context("recorded reimbursement succeeds")?;
    // Both pending-commit PDAs close into the recorded identity.
    assert_eq!(
        ctx.banks_client.get_balance(validator).await?,
        validator_before + 2 * LAMPORTS
    );
    assert_eq!(ctx.banks_client.get_balance(attacker).await?, 1);
    Ok(())
}

#[tokio::test]
async fn rollback_before_the_request_expires_is_rejected_without_mutation() -> anyhow::Result<()> {
    let env = env(u64::MAX, None)?;
    let mut ctx = start_with_dlp(env.accounts.clone()).await;
    let keys = all_keys(&env);
    let before = snapshot(&mut ctx, &keys).await?;
    let err = send(
        &mut ctx,
        rollback_ix(&env, env.rent_payer, Pubkey::new_unique()),
        &[],
    )
    .await
    .err()
    .context("unexpired request must fail")?;
    assert_eq!(
        custom_code(&err),
        Some(DlpError::UndelegationRequestNotExpired as u32)
    );
    assert_eq!(snapshot(&mut ctx, &keys).await?, before);
    Ok(())
}

#[tokio::test]
async fn rollback_rejects_another_games_metadata_and_a_repeat() -> anyhow::Result<()> {
    let env = env(0, None)?;
    let mut ctx = start_with_dlp(env.accounts.clone()).await;
    let mut swapped = rollback_ix(&env, env.rent_payer, Pubkey::new_unique());
    swapped.accounts[4].pubkey = pdas_for(&game_pda(GAME_ID + 1).0).metadata;
    let keys = all_keys(&env);
    let before = snapshot(&mut ctx, &keys).await?;
    send(&mut ctx, swapped, &[])
        .await
        .err()
        .context("another game's metadata PDA must fail")?;
    assert_eq!(snapshot(&mut ctx, &keys).await?, before);

    send(
        &mut ctx,
        rollback_ix(&env, env.rent_payer, Pubkey::new_unique()),
        &[],
    )
    .await
    .context("first rollback")?;
    ctx.last_blockhash = ctx.banks_client.get_latest_blockhash().await?;
    let after_first = snapshot(&mut ctx, &keys).await?;
    send(
        &mut ctx,
        rollback_ix(&env, env.rent_payer, Pubkey::new_unique()),
        &[],
    )
    .await
    .err()
    .context("a second rollback must fail")?;
    assert_eq!(snapshot(&mut ctx, &keys).await?, after_first);
    Ok(())
}
