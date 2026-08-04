//! Live-devnet drills for paths `game_flows.rs`'s happy-path runs never
//! exercise: whether MagicBlock's scheduler actually invokes a cranked
//! instruction *autonomously* (as opposed to a client calling it directly,
//! which is all `run_1v1_game_flow`'s Step 8 proves), and the entire
//! ER-unavailability recovery chain (`request_force_undelegate` ->
//! `force_undelegate_after_timeout` -> `recover_stuck_delegation`).
//!
//! Both are opt-in (`--mode crank-drill` / `--mode recovery-drill`), never
//! part of the default benchmark run:
//! - The crank drill costs ~1-2 real minutes.
//! - The recovery drill costs ~60-70 real minutes (the delegation program's
//!   own `DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS`, not something
//!   client-side code can shorten) plus real devnet SOL moved through the
//!   exact auto-recovery path `backend/src/tasks/settlement_worker.rs` now
//!   runs unattended in production — run this deliberately, not casually.

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

use crate::{apply_compute_budget, instructions as ix, unique_id, with_retry};

const GAME_SEED: &[u8] = b"game";
const WAGER_ESCROW_SEED: &[u8] = b"escrow";

/// `GameStatus` discriminants (borsh enum tags, `state/game.rs`'s
/// declaration order) — mirrors `backend/src/tasks/settlement_worker.rs`'s
/// `parse_game_account` constants, duplicated here since this crate can't
/// depend on the backend or the program crate.
const STATUS_FINISHED: u8 = 5;

/// Sets up a game through delegation with the given clock/increment, mirroring
/// `run_1v1_game_flow`'s steps 1-5 exactly (profile init tolerant of
/// already-exists, create, join, authorize + fund session keys, delegate).
/// Returns `(game_id, game_pda, session_white, session_black)`.
async fn setup_delegated_game(
    base_rpc: &RpcClient,
    program_id: Pubkey,
    white: &Keypair,
    black: &Keypair,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> anyhow::Result<(u64, Pubkey, Keypair, Keypair)> {
    let game_id = unique_id();
    let suffix = game_id & 0xFFFF;

    for (payer, username) in [
        (white, format!("dw_{:04x}", suffix)),
        (black, format!("db_{:04x}", suffix)),
    ] {
        let profile_ix = ix::init_profile_ix(
            program_id,
            payer.pubkey(),
            username,
            "GB".to_string(),
            631_152_000,
        )?;
        let mut ixs = vec![profile_ix];
        apply_compute_budget(&mut ixs, 200_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx =
            Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[payer], blockhash);
        if let Err(e) = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await {
            let msg = e.to_string();
            if !(msg.contains("0x1773") || msg.contains("already") || msg.contains("AlreadyInUse"))
            {
                return Err(e);
            }
        }
    }

    let mut create_ixs = vec![ix::create_game_ix(
        program_id,
        white.pubkey(),
        white.pubkey(),
        game_id,
        1_000_000, // wager_amount (lamports) - small, real, on devnet
        1,
        0, // platform_fee
        base_time_seconds,
        increment_seconds,
    )?];
    apply_compute_budget(&mut create_ixs, 300_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&create_ixs, Some(&white.pubkey()), &[white], blockhash);
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;

    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut join_ixs = vec![ix::join_game_ix(
        program_id,
        black.pubkey(),
        white.pubkey(),
        white.pubkey(),
        game_id,
    )?];
    apply_compute_budget(&mut join_ixs, 200_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &join_ixs,
        Some(&black.pubkey()),
        &[black, white],
        blockhash,
    );
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;

    let session_white = Keypair::new();
    let session_black = Keypair::new();
    for (payer, session) in [(white, &session_white), (black, &session_black)] {
        let mut auth_ixs = vec![ix::authorize_session_key_ix(
            program_id,
            payer.pubkey(),
            game_id,
            session.pubkey(),
        )?];
        apply_compute_budget(&mut auth_ixs, 150_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &auth_ixs,
            Some(&payer.pubkey()),
            &[payer],
            blockhash,
        );
        with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    }

    let blockhash = base_rpc.get_latest_blockhash()?;
    let white_fund_ix =
        system_instruction::transfer(&white.pubkey(), &session_white.pubkey(), 20_000_000);
    let black_fund_ix =
        system_instruction::transfer(&black.pubkey(), &session_black.pubkey(), 20_000_000);
    let tx_white = Transaction::new_signed_with_payer(
        &[white_fund_ix],
        Some(&white.pubkey()),
        &[white],
        blockhash,
    );
    let tx_black = Transaction::new_signed_with_payer(
        &[black_fund_ix],
        Some(&black.pubkey()),
        &[black],
        blockhash,
    );
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx_white)).await?;
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx_black)).await?;

    let mut delegate_ixs = vec![ix::delegate_game_ix(
        program_id,
        game_pda,
        white.pubkey(),
        white.pubkey(),
        game_id,
        7200,
    )?];
    apply_compute_budget(&mut delegate_ixs, 300_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &delegate_ixs,
        Some(&white.pubkey()),
        &[white],
        blockhash,
    );
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;

    // Give the ER validator time to pick up the delegation - see the same
    // wait in `game_flows.rs::run_1v1_game_flow`.
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    Ok((game_id, game_pda, session_white, session_black))
}

/// Proves MagicBlock's scheduler autonomously invokes `crank_time_check`
/// (as opposed to a client calling it directly). Delegates a game with a 5s
/// chess clock, schedules the crank at a 5s interval, waits, and reads the
/// Game PDA back from the ER **without this drill ever building or sending
/// a `crank_time_check` instruction itself** - if the game shows
/// `status == Finished` anyway, only MagicBlock's own scheduler could have
/// gotten it there.
pub async fn run_crank_liveness_drill(
    base_rpc: &RpcClient,
    er_rpc: &RpcClient,
    program_id: Pubkey,
    white: &Keypair,
    black: &Keypair,
) -> anyhow::Result<()> {
    println!("\n   [crank-drill] Setting up a game with a 5s clock...");
    let (game_id, game_pda, session_white, _session_black) =
        setup_delegated_game(base_rpc, program_id, white, black, 5, 0).await?;
    println!("   [crank-drill] game #{game_id}, PDA {game_pda}");

    println!("   [crank-drill] Scheduling time-check crank at a 5s interval (never calling crank_time_check_ix ourselves)...");
    let schedule_ix = ix::schedule_time_check_ix(
        program_id,
        game_pda,
        session_white.pubkey(),
        white.pubkey(),
        black.pubkey(),
        game_id,
        5_000,
    )?;
    let mut schedule_ixs = vec![schedule_ix];
    apply_compute_budget(&mut schedule_ixs, 150_000, 10_000, 262_144);
    let blockhash = crate::get_blockhash_for_accounts(er_rpc, &[game_pda])?;
    let tx = Transaction::new_signed_with_payer(
        &schedule_ixs,
        Some(&session_white.pubkey()),
        &[&session_white],
        blockhash,
    );
    with_retry(|| er_rpc.send_and_confirm_transaction(&tx)).await?;

    println!("   [crank-drill] Waiting up to 90s for MagicBlock's scheduler to fire the crank on its own...");
    let start = std::time::Instant::now();
    loop {
        if let Ok(account) = er_rpc.get_account(&game_pda) {
            // status sits at offset 8 (disc) + 8 (game_id) + 32 (white) + 32 (black).
            if let Some(&status) = account.data.get(80) {
                if status == STATUS_FINISHED {
                    println!(
                        "   [crank-drill] PASS - game reached Finished after {:?} with no manual crank call. \
                         MagicBlock's scheduler is autonomously invoking crank_time_check.",
                        start.elapsed()
                    );
                    return Ok(());
                }
            }
        }
        if start.elapsed() > std::time::Duration::from_secs(90) {
            anyhow::bail!(
                "crank-drill FAIL: game {game_id} did not reach Finished within 90s with no \
                 manual crank_time_check call - either the scheduler never fired, or the \
                 schedule_time_check registration itself failed silently. Check the ER \
                 explorer for game PDA {game_pda}."
            );
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

/// Exercises the full ER-unavailability recovery chain against the real
/// delegation program on devnet: delegate -> deliberately never undelegate
/// -> `request_force_undelegate` -> wait out the real ~60min timeout ->
/// `force_undelegate_after_timeout` (asserts the Game PDA really does come
/// back wiped to zero bytes, matching the on-chain doc comments' claim) ->
/// `recover_stuck_delegation` (asserts both wallets' balances actually move
/// by escrow/2 each). This is the first time this path runs against a real
/// validator instead of `solana-program-test`'s mocked delegation program.
///
/// `dispute_authority` must be the real keypair matching
/// `constants::dispute_authority::ID` on-chain (see
/// `keygen::load_dispute_authority_keypair`) - a wrong key fails the last
/// step's signer constraint, not silently.
pub async fn run_stuck_delegation_drill(
    base_rpc: &RpcClient,
    program_id: Pubkey,
    white: &Keypair,
    black: &Keypair,
    dispute_authority: &Keypair,
) -> anyhow::Result<()> {
    println!("\n   [recovery-drill] Setting up a game and delegating (normal 600s clock)...");
    let (game_id, game_pda, _session_white, _session_black) =
        setup_delegated_game(base_rpc, program_id, white, black, 600, 0).await?;
    println!(
        "   [recovery-drill] game #{game_id}, PDA {game_pda} - deliberately NOT undelegating."
    );

    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let white_before = with_retry(|| base_rpc.get_balance(&white.pubkey())).await?;
    let black_before = with_retry(|| base_rpc.get_balance(&black.pubkey())).await?;
    let escrow_before = with_retry(|| base_rpc.get_balance(&escrow_pda)).await?;
    println!("   [recovery-drill] Escrow before: {escrow_before} lamports");

    println!("   [recovery-drill] Requesting force-undelegation (starts the ~60min countdown)...");
    let req_ix = ix::request_force_undelegate_ix(program_id, game_id, white.pubkey())?;
    let mut req_ixs = vec![req_ix];
    apply_compute_budget(&mut req_ixs, 200_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&req_ixs, Some(&white.pubkey()), &[white], blockhash);
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;

    println!("   [recovery-drill] Polling the UndelegationRequest PDA's expiry against the real clock (~60min - this is the real wait, not simulated)...");
    let undelegation_request = Pubkey::new_from_array(
        ephemeral_rollups_sdk::pda::undelegation_request_pda_from_delegated_account(
            &game_pda.to_bytes().into(),
        )
        .to_bytes(),
    );
    loop {
        let account = base_rpc.get_account(&undelegation_request)?;
        // 8-byte disc + 32-byte delegated_account + 8-byte expires_at_slot (LE).
        let expires_at_slot = u64::from_le_bytes(account.data[40..48].try_into()?);
        let current_slot = base_rpc.get_slot()?;
        if current_slot >= expires_at_slot {
            println!("   [recovery-drill] Timeout window elapsed (slot {current_slot} >= {expires_at_slot}).");
            break;
        }
        println!(
            "   [recovery-drill] Not yet ({current_slot}/{expires_at_slot}) - sleeping 60s..."
        );
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }

    println!("   [recovery-drill] Completing force-undelegation (Game PDA will be wiped to zero bytes)...");
    let force_ix = ix::force_undelegate_after_timeout_ix(program_id, game_id, white.pubkey())?;
    let mut force_ixs = vec![force_ix];
    apply_compute_budget(&mut force_ixs, 250_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&force_ixs, Some(&white.pubkey()), &[white], blockhash);
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;

    let wiped = base_rpc.get_account(&game_pda)?;
    if wiped.owner != program_id || !wiped.data.is_empty() {
        anyhow::bail!(
            "recovery-drill FAIL: expected Game PDA owned by program with empty data after \
             force_undelegate_after_timeout, got owner={} data_len={}",
            wiped.owner,
            wiped.data.len()
        );
    }
    println!("   [recovery-drill] Confirmed: Game PDA is program-owned with 0 bytes of data.");

    println!("   [recovery-drill] Recovering escrow via recover_stuck_delegation...");
    let recover_ix = ix::recover_stuck_delegation_ix(
        program_id,
        game_id,
        white.pubkey(),
        black.pubkey(),
        dispute_authority.pubkey(),
    );
    let mut recover_ixs = vec![recover_ix];
    apply_compute_budget(&mut recover_ixs, 200_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &recover_ixs,
        Some(&dispute_authority.pubkey()),
        &[dispute_authority],
        blockhash,
    );
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;

    let white_after = with_retry(|| base_rpc.get_balance(&white.pubkey())).await?;
    let black_after = with_retry(|| base_rpc.get_balance(&black.pubkey())).await?;
    let white_delta = white_after as i64 - white_before as i64;
    let black_delta = black_after as i64 - black_before as i64;
    let expected_each = (escrow_before / 2) as i64;

    println!(
        "   [recovery-drill] White delta: {white_delta} lamports, Black delta: {black_delta} lamports (expected ~{expected_each} each)"
    );

    // Allow for tx fees eating into the deltas - just confirm both moved up
    // by a materially positive amount close to escrow/2, not an exact match.
    if white_delta < expected_each / 2 || black_delta < expected_each / 2 {
        anyhow::bail!(
            "recovery-drill FAIL: escrow split did not land as expected (white {white_delta}, \
             black {black_delta}, expected ~{expected_each} each)"
        );
    }

    println!("   [recovery-drill] PASS - full ER-unavailability recovery chain verified against real devnet.");
    Ok(())
}
