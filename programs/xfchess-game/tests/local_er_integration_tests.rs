//! Real local L1 + ER test; all signing keys are generated in memory.
//! Fixed loopback URLs prevent this target from ever touching devnet/mainnet.
use anchor_lang::{AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas};
use anyhow::{ensure, Context as _};
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use xfchess_game::state::{Game, GameStatus, MatchType, RecoveryIdentity, RecoveryStatus};

fn submit(
    rpc: &RpcClient,
    payer: &Keypair,
    extra: &[&Keypair],
    ix: Instruction,
) -> anyhow::Result<()> {
    let mut signers = vec![payer];
    signers.extend_from_slice(extra);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &signers,
        rpc.get_latest_blockhash()?,
    );
    let simulation = rpc
        .simulate_transaction(&xfchess_client_compat::rpc_transaction(&tx)?)?
        .value;
    ensure!(
        simulation.err.is_none(),
        "simulation failed: {:?}; logs: {:?}",
        simulation.err,
        simulation.logs
    );
    println!("simulation CU: {:?}", simulation.units_consumed);
    let signature =
        rpc.send_and_confirm_transaction(&xfchess_client_compat::rpc_transaction(&tx)?)?;
    println!("local signature: {signature}");
    Ok(())
}

fn wait_account(
    rpc: &RpcClient,
    key: &Pubkey,
    predicate: impl Fn(&Account) -> bool,
) -> anyhow::Result<Account> {
    let start = Instant::now();
    loop {
        if let Ok(account) = rpc.get_account(key) {
            let account = Account {
                lamports: account.lamports,
                data: account.data,
                owner: account.owner,
                executable: account.executable,
                rent_epoch: account.rent_epoch,
            };
            if predicate(&account) {
                return Ok(account);
            }
        }
        ensure!(
            start.elapsed() < Duration::from_secs(45),
            "account {key} did not reach expected state"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn ix(accounts: Vec<AccountMeta>, data: Vec<u8>) -> Instruction {
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

#[test]
#[ignore = "requires the explicitly started local L1/ER stack on ports 18899/17799"]
fn delegate_move_undelegate_restores_exact_game_bytes_and_retains_identity() -> anyhow::Result<()> {
    use ephemeral_rollups_sdk::pda::{
        delegate_buffer_pda_from_delegated_account_and_owner_program,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    };
    let base = RpcClient::new_with_timeout_and_commitment(
        "http://127.0.0.1:18899",
        Duration::from_secs(25),
        CommitmentConfig::confirmed(),
    );
    let er = RpcClient::new_with_timeout_and_commitment(
        "http://127.0.0.1:17799",
        Duration::from_secs(25),
        CommitmentConfig::confirmed(),
    );
    ensure!(
        base.get_health().is_ok() && er.get_health().is_ok(),
        "start the local stack first"
    );
    let payer = Keypair::new();
    let white = Keypair::new();
    let black = Keypair::new();
    let session_key = Keypair::new();
    let airdrop = base.request_airdrop(&payer.pubkey(), 10_000_000_000)?;
    let started = Instant::now();
    while !base.confirm_transaction(&airdrop)? {
        ensure!(
            started.elapsed() < Duration::from_secs(30),
            "local airdrop timed out"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    let game_id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let program = xfchess_game::ID;
    let game = Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program).0;
    let escrow = Pubkey::find_program_address(&[b"escrow", &game_id.to_le_bytes()], &program).0;
    let identity =
        Pubkey::find_program_address(&[RecoveryIdentity::SEED, &game_id.to_le_bytes()], &program).0;
    let profile =
        |player: &Pubkey| Pubkey::find_program_address(&[b"profile", player.as_ref()], &program).0;
    for (index, player) in [&white, &black].iter().enumerate() {
        submit(
            &base,
            &payer,
            &[],
            solana_system_interface::instruction::transfer(
                &payer.pubkey(),
                &player.pubkey(),
                2_000_000_000,
            ),
        )?;
        let username = format!("er{index}{:x}", game_id);
        submit(
            &base,
            &payer,
            &[player],
            ix(
                xfchess_game::accounts::InitProfile {
                    player_profile: profile(&player.pubkey()),
                    username_record: Pubkey::find_program_address(
                        &[b"username", username.as_bytes()],
                        &program,
                    )
                    .0,
                    player: player.pubkey(),
                    system_program: solana_system_interface::program::ID,
                }
                .to_account_metas(None),
                xfchess_game::instruction::InitProfile {
                    username,
                    country: "GB".into(),
                    date_of_birth: 631_152_000,
                }
                .data(),
            ),
        )?;
    }
    submit(
        &base,
        &payer,
        &[&white],
        ix(
            xfchess_game::accounts::CreateGameV2 {
                legacy: xfchess_game::accounts::CreateGame {
                    game,
                    escrow_pda: escrow,
                    player: white.pubkey(),
                    fee_payer: payer.pubkey(),
                    system_program: solana_system_interface::program::ID,
                },
                recovery_identity: identity,
            }
            .to_account_metas(None),
            xfchess_game::instruction::CreateGameV2 {
                game_id,
                wager_amount: 10_000_000,
                match_type: MatchType::Rated,
                platform_fee: 0,
                base_time_seconds: 300,
                increment_seconds: 0,
            }
            .data(),
        ),
    )?;
    submit(
        &base,
        &payer,
        &[&black],
        ix(
            xfchess_game::accounts::JoinGameV2 {
                legacy: xfchess_game::accounts::JoinGame {
                    game,
                    player_profile: profile(&black.pubkey()),
                    escrow_pda: escrow,
                    white_profile: profile(&white.pubkey()),
                    player: black.pubkey(),
                    fee_payer: payer.pubkey(),
                    system_program: solana_system_interface::program::ID,
                },
                recovery_identity: identity,
            }
            .to_account_metas(None),
            xfchess_game::instruction::JoinGameV2 { game_id }.data(),
        ),
    )?;
    let session = Pubkey::find_program_address(
        &[
            b"session_delegation",
            &game_id.to_le_bytes(),
            white.pubkey().as_ref(),
        ],
        &program,
    )
    .0;
    submit(
        &base,
        &payer,
        &[&white],
        ix(
            xfchess_game::accounts::AuthorizeSessionCtx {
                game,
                session_delegation: session,
                player: white.pubkey(),
                system_program: solana_system_interface::program::ID,
            }
            .to_account_metas(None),
            xfchess_game::instruction::AuthorizeSessionKey {
                game_id,
                session_pubkey: session_key.pubkey(),
            }
            .data(),
        ),
    )?;
    let buffer = delegate_buffer_pda_from_delegated_account_and_owner_program(
        &game.to_bytes().into(),
        &program.to_bytes().into(),
    );
    let record = delegation_record_pda_from_delegated_account(&game.to_bytes().into());
    let metadata = delegation_metadata_pda_from_delegated_account(&game.to_bytes().into());
    submit(
        &base,
        &payer,
        &[],
        ix(
            xfchess_game::accounts::DelegateGameV2 {
                legacy: xfchess_game::accounts::DelegateGameCtx {
                    game,
                    payer: payer.pubkey(),
                    owner_program: program,
                    buffer: Pubkey::new_from_array(buffer.to_bytes()),
                    delegation_record: Pubkey::new_from_array(record.to_bytes()),
                    delegation_metadata: Pubkey::new_from_array(metadata.to_bytes()),
                    delegation_program: ephemeral_rollups_sdk::id(),
                    system_program: solana_system_interface::program::ID,
                    fee_payer: payer.pubkey(),
                },
                recovery_identity: identity,
            }
            .to_account_metas(None),
            xfchess_game::instruction::DelegateGameV2 {
                game_id,
                valid_until: 600,
            }
            .data(),
        ),
    )?;
    let delegated = base.get_account(&game)?;
    ensure!(
        delegated.owner == ephemeral_rollups_sdk::id(),
        "base Game ownership did not delegate"
    );
    let identity_before = base.get_account(&identity)?;
    ensure!(
        RecoveryIdentity::try_deserialize(&mut identity_before.data.as_slice())?.status
            == RecoveryStatus::Delegated,
        "record not sealed before delegation"
    );
    let er_game = wait_account(&er, &game, |a| {
        a.owner == program && Game::try_deserialize(&mut a.data.as_slice()).is_ok()
    })?;
    let mut state = Game::try_deserialize(&mut er_game.data.as_slice())?;
    ensure!(
        state.status == GameStatus::Active && state.is_delegated,
        "ER did not clone the active Game"
    );
    // Apply one legal move using the exact on-chain engine and the real session signature.
    let move_uci = *b"e2e4\0";
    let mut board =
        xfchess_game::chess_logic_on_chain::CompactBoard::from_bytes(&state.board_state)
            .to_on_chain_game();
    xfchess_game::chess_logic_on_chain::validate_and_apply(&mut board, &move_uci)
        .map_err(|_| anyhow::anyhow!("fixture move invalid"))?;
    let next_board = board.to_compact_board().to_bytes();
    submit(
        &er,
        &payer,
        &[&session_key],
        ix(
            xfchess_game::accounts::RecordMove {
                game,
                player: session_key.pubkey(),
                session_delegation: session,
            }
            .to_account_metas(None),
            xfchess_game::instruction::RecordMove {
                game_id,
                move_uci,
                next_board,
                nonce: 1,
                signature: None,
                parent_nonce: Some(0),
            }
            .data(),
        ),
    )?;
    let moved = er.get_account(&game)?;
    state = Game::try_deserialize(&mut moved.data.as_slice())?;
    ensure!(
        state.nonce == 1 && state.board_state == next_board,
        "ER move did not apply"
    );
    let stale_ix = ix(
        xfchess_game::accounts::RecordMove {
            game,
            player: session_key.pubkey(),
            session_delegation: session,
        }
        .to_account_metas(None),
        xfchess_game::instruction::RecordMove {
            game_id,
            move_uci,
            next_board,
            nonce: 1,
            signature: None,
            parent_nonce: Some(0),
        }
        .data(),
    );
    let stale_tx = Transaction::new_signed_with_payer(
        &[stale_ix],
        Some(&payer.pubkey()),
        &[&payer, &session_key],
        er.get_latest_blockhash()?,
    );
    ensure!(
        er.simulate_transaction(&xfchess_client_compat::rpc_transaction(&stale_tx)?)?
            .value
            .err
            .is_some(),
        "stale ER move unexpectedly accepted"
    );
    ensure!(
        er.get_account(&game)?.data == moved.data,
        "stale simulation changed state"
    );
    submit(
        &er,
        &payer,
        &[],
        ix(
            xfchess_game::accounts::ScheduleTimeCheck {
                payer: payer.pubkey(),
                game,
                white: white.pubkey(),
                black: black.pubkey(),
                magic_program: ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID,
            }
            .to_account_metas(None),
            xfchess_game::instruction::ScheduleTimeCheck {
                args: xfchess_game::crank_ix::schedule_time_check::ScheduleTimeCheckArgs {
                    task_id: game_id,
                    check_interval_millis: 60_000,
                    iterations: 1,
                },
            }
            .data(),
        ),
    )?;
    submit(
        &er,
        &payer,
        &[],
        ix(
            xfchess_game::accounts::CancelTimeCheck {
                payer: payer.pubkey(),
                game,
                magic_program: ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID,
            }
            .to_account_metas(None),
            xfchess_game::instruction::CancelTimeCheck {
                args: xfchess_game::crank_ix::cancel_time_check::CancelTimeCheckArgs {
                    task_id: game_id,
                },
            }
            .data(),
        ),
    )?;
    ensure!(
        er.get_account(&game)?.data == moved.data,
        "scheduling changed Game bytes"
    );
    submit(
        &er,
        &payer,
        &[],
        ix(
            xfchess_game::accounts::UndelegateGameCtx {
                game,
                payer: payer.pubkey(),
                magic_context: ephemeral_rollups_sdk::consts::MAGIC_CONTEXT_ID,
                magic_program: ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID,
            }
            .to_account_metas(None),
            xfchess_game::instruction::UndelegateGame { game_id }.data(),
        ),
    )?;
    state.is_delegated = false;
    state.fees_advanced = state
        .fees_advanced
        .checked_add(xfchess_game::constants::UNDELEGATE_COST)
        .and_then(|value| value.checked_add(xfchess_game::constants::ER_SESSION_FEE_LAMPORTS))
        .context("expected undelegation fees overflowed")?;
    let mut expected = Vec::new();
    state.try_serialize(&mut expected)?;
    expected.resize(moved.data.len(), 0);
    let restored = wait_account(&base, &game, |a| a.owner == program && a.data == expected)?;
    ensure!(
        restored.data == expected,
        "restored bytes differ from committed ER state"
    );
    ensure!(
        base.get_account(&identity)? == identity_before,
        "recovery record changed during ER execution"
    );
    ensure!(
        base.get_balance(&escrow)? == 20_000_000,
        "ER round-trip changed wager escrow"
    );
    println!("PASS: real delegate -> legal move -> stale rejection -> schedule/cancel -> commit/undelegate -> exact byte restoration; game_id={game_id}");
    Ok(())
}
