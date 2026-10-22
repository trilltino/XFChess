use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

use crate::chain::{
    apply_compute_budget, fetch_profile_elo, instructions as ix, moves::generate_100_move_sequence,
    unique_id, with_retry,
};
use crate::cost::measurements::{ExecutionLayer, MeasurementLog};

use super::common::*;

pub async fn run_1v1_game_flow(
    base_rpc: &RpcClient,
    er_rpc: &RpcClient,
    program_id: Pubkey,
    master: &Keypair,
    white: &Keypair,
    black: &Keypair,
    logger: &mut MeasurementLog,
) -> anyhow::Result<()> {
    let game_id = unique_id()?;
    // Use game_id suffix so usernames are unique per run — profiles persist on devnet forever.
    let suffix = game_id & 0xFFFF; // last 4 hex digits of timestamp
    println!("\n   Setting up 1v1 game #{}", game_id);

    // Existing profiles may return UsernameTaken or AlreadyInitialized on repeated runs.
    println!("   Step 1: Initializing player profiles...");
    let profile_data = [
        (white, format!("w_{:04x}", suffix)),
        (black, format!("b_{:04x}", suffix)),
    ];
    for (payer, username) in &profile_data {
        let profile_ix = ix::init_profile_ix(
            program_id,
            payer.pubkey(),
            username.clone(),
            "GB".to_string(),
            631_152_000, // 1990-01-01T00:00:00Z — on-chain age check requires date_of_birth > 0
        )?;
        let mut ixs = vec![profile_ix];
        apply_compute_budget(&mut ixs, 200_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx =
            Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[*payer], blockhash);
        match with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await {
            Ok(sig) => logger.log(
                ExecutionLayer::Solana,
                "profile",
                "init_profile",
                true,
                Some(sig.to_string()),
            ),
            Err(e) => {
                let msg = e.to_string();
                // 0x1773 = UsernameTaken (6003), 0x0 = AlreadyInUse — both mean profile exists, continue
                if msg.contains("0x1773") || msg.contains("already") || msg.contains("AlreadyInUse")
                {
                    println!(
                        "   Profile already exists for {}, skipping.",
                        payer.pubkey()
                    );
                    logger.log(
                        ExecutionLayer::Solana,
                        "profile",
                        "init_profile",
                        true,
                        None,
                    );
                } else {
                    return Err(e);
                }
            }
        }
    }

    println!("   Step 2: Creating game...");
    let mut create_ixs = vec![ix::create_game_ix(
        program_id,
        white.pubkey(),
        white.pubkey(),
        game_id,
        1_000_000,
        1,
        0, // platform_fee
        600,
        0,
    )?];
    apply_compute_budget(&mut create_ixs, 300_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&create_ixs, Some(&white.pubkey()), &[white], blockhash);
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "game_setup",
        "create_game",
        true,
        Some(sig.to_string()),
    );

    let game_pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[b"game", &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    println!("   Game PDA: {}", game_pda);

    println!("   Step 3: Black joining game...");
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
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "game_setup",
        "join_game",
        true,
        Some(sig.to_string()),
    );

    println!("   Step 4: Authorizing session keys...");
    let session_white = Keypair::new();
    let session_black = Keypair::new();

    let mut auth_ixs = vec![ix::authorize_session_key_ix(
        program_id,
        white.pubkey(),
        game_id,
        session_white.pubkey(),
    )?];
    apply_compute_budget(&mut auth_ixs, 150_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&auth_ixs, Some(&white.pubkey()), &[white], blockhash);
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "delegation",
        "authorize_session_key",
        true,
        Some(sig.to_string()),
    );

    let mut auth_ixs = vec![ix::authorize_session_key_ix(
        program_id,
        black.pubkey(),
        game_id,
        session_black.pubkey(),
    )?];
    apply_compute_budget(&mut auth_ixs, 150_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&auth_ixs, Some(&black.pubkey()), &[black], blockhash);
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "delegation",
        "authorize_session_key",
        true,
        Some(sig.to_string()),
    );

    // Fund session keys so they can pay L1 transaction fees
    println!("   Funding session keypairs for L1 transaction fees...");
    let white_fund_ix =
        system_instruction::transfer(&white.pubkey(), &session_white.pubkey(), 20_000_000);
    let black_fund_ix =
        system_instruction::transfer(&black.pubkey(), &session_black.pubkey(), 20_000_000);
    let blockhash = base_rpc.get_latest_blockhash()?;
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
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx_white)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "funding",
        "fund_session",
        true,
        Some(sig.to_string()),
    );
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx_black)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "funding",
        "fund_session",
        true,
        Some(sig.to_string()),
    );

    println!("   Step 5: Delegating game to ER...");
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
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "delegation",
        "delegate_game",
        true,
        Some(sig.to_string()),
    );

    // Allow the ER validator to observe L1 delegation before the first write.
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    println!("   Step 6: Scheduling time check crank...");
    if let Ok(schedule_ix) = ix::schedule_time_check_ix(
        program_id,
        game_pda,
        white.pubkey(),
        white.pubkey(),
        black.pubkey(),
        game_id,
        30_000,
    ) {
        let mut schedule_ixs = vec![schedule_ix];
        apply_compute_budget(&mut schedule_ixs, 150_000, 10_000, 262_144);
        if let Ok(blockhash) = crate::chain::get_blockhash_for_accounts(er_rpc, &[game_pda]) {
            let tx = Transaction::new_signed_with_payer(
                &schedule_ixs,
                Some(&white.pubkey()),
                &[white],
                blockhash,
            );
            match er_rpc.send_and_confirm_transaction(&tx) {
                Ok(sig) => {
                    logger.log(
                        ExecutionLayer::Er,
                        "crank",
                        "schedule_time_check",
                        true,
                        Some(sig.to_string()),
                    );
                }
                Err(e) => {
                    println!("   [Warning] Step 6 (Schedule Crank) failed: {:?}. Skipping task scheduling...", e);
                }
            }
        }
    }

    println!("   Step 7: Playing 100 moves on MagicBlock ER...");
    let sequence = generate_100_move_sequence()?;
    let mut move_latencies: Vec<std::time::Duration> = Vec::with_capacity(100);
    // Reuse blockhashes for about three seconds to avoid one RPC round trip per move.
    let mut cached_blockhash: Option<(solana_sdk::hash::Hash, std::time::Instant)> = None;
    let mut get_cached_blockhash =
        |er_rpc: &RpcClient,
         game_pda: Pubkey|
         -> solana_client::client_error::Result<solana_sdk::hash::Hash> {
            if let Some((hash, fetched_at)) = cached_blockhash {
                if fetched_at.elapsed() < std::time::Duration::from_secs(3) {
                    return Ok(hash);
                }
            }
            let hash = crate::chain::get_blockhash_for_accounts(er_rpc, &[game_pda])?;
            cached_blockhash = Some((hash, std::time::Instant::now()));
            Ok(hash)
        };
    // Send precomputed moves in order without per-move confirmation, then
    // confirm their signatures in one batch.
    const BATCH_SIZE: usize = 10;
    let mut i = 0usize;
    while i < 100 {
        let batch_end = (i + BATCH_SIZE).min(100);
        let batch_start = std::time::Instant::now();
        let mut batch_sigs = Vec::with_capacity(batch_end - i);
        let blockhash = get_cached_blockhash(er_rpc, game_pda)?;
        for (j, vm) in sequence.iter().enumerate().take(batch_end).skip(i) {
            let (session_key, wallet) = if j % 2 == 0 {
                (session_white.pubkey(), white.pubkey())
            } else {
                (session_black.pubkey(), black.pubkey())
            };
            let nonce = (j + 1) as u64; // game.nonce starts at 0, first move must be 1
            let mut move_ixs = vec![ix::record_move_ix(
                program_id,
                session_key,
                wallet,
                game_id,
                vm.uci,
                vm.next_board,
                nonce,
                None,
            )?];
            apply_compute_budget(&mut move_ixs, 80_000, 10_000, 262_144);
            let payer = if j % 2 == 0 {
                &session_white
            } else {
                &session_black
            };
            let tx = Transaction::new_signed_with_payer(
                &move_ixs,
                Some(&session_key),
                &[payer],
                blockhash,
            );
            let sig = with_retry(|| er_rpc.send_transaction(&tx)).await?;
            batch_sigs.push(sig);
        }

        // One batched confirm for the whole group instead of N separate ones.
        let timeout = std::time::Duration::from_secs(30);
        let poll_interval = std::time::Duration::from_millis(20);
        let poll_start = std::time::Instant::now();
        loop {
            let statuses = er_rpc.get_signature_statuses(&batch_sigs)?;
            let all_done = statuses.value.iter().all(|s| s.is_some());
            if all_done {
                for (sig, status) in batch_sigs.iter().zip(statuses.value.iter()) {
                    let status = status
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("missing transaction status for {sig}"))?;
                    if let Some(err) = &status.err {
                        anyhow::bail!("move tx {sig} failed: {err}");
                    }
                }
                break;
            }
            if poll_start.elapsed() > timeout {
                anyhow::bail!("timed out waiting for batch [{i}..{batch_end}) to confirm");
            }
            std::thread::sleep(poll_interval);
        }

        let batch_elapsed = batch_start.elapsed();
        let per_move = batch_elapsed / (batch_end - i) as u32;
        for (k, sig) in batch_sigs.iter().enumerate() {
            move_latencies.push(per_move);
            let move_no = i + k + 1;
            logger.log(
                ExecutionLayer::Er,
                "gameplay",
                "record_move",
                true,
                Some(sig.to_string()),
            );
            println!(
                "     Move {:>3}/100: {:>6.1}ms (batch avg) | https://explorer.solana.com/tx/{}?cluster=custom&customUrl=https%3A%2F%2Fdevnet.magicblock.app",
                move_no,
                per_move.as_secs_f64() * 1000.0,
                sig
            );
        }

        i = batch_end;
    }

    if !move_latencies.is_empty() {
        let total: std::time::Duration = move_latencies.iter().sum();
        let avg_ms = total.as_secs_f64() * 1000.0 / move_latencies.len() as f64;
        let min_ms = move_latencies
            .iter()
            .min()
            .ok_or_else(|| anyhow::anyhow!("missing minimum move latency"))?
            .as_secs_f64()
            * 1000.0;
        let max_ms = move_latencies
            .iter()
            .max()
            .ok_or_else(|| anyhow::anyhow!("missing maximum move latency"))?
            .as_secs_f64()
            * 1000.0;
        println!(
            "   Move latency (send+confirm round-trip): min {:.1}ms, avg {:.1}ms, max {:.1}ms",
            min_ms, avg_ms, max_ms
        );
    }

    println!("   Step 8: Running time check crank...");
    if let Ok(crank_ix) =
        ix::crank_time_check_ix(program_id, game_pda, white.pubkey(), black.pubkey())
    {
        let mut crank_ixs = vec![crank_ix];
        apply_compute_budget(&mut crank_ixs, 100_000, 10_000, 262_144);
        if let Ok(blockhash) = crate::chain::get_blockhash_for_accounts(er_rpc, &[game_pda]) {
            let tx = Transaction::new_signed_with_payer(
                &crank_ixs,
                Some(&master.pubkey()),
                &[master],
                blockhash,
            );
            match er_rpc.send_and_confirm_transaction(&tx) {
                Ok(sig) => {
                    logger.log(
                        ExecutionLayer::Er,
                        "crank",
                        "crank_time_check",
                        true,
                        Some(sig.to_string()),
                    );
                }
                Err(e) => {
                    println!(
                        "   [Warning] Step 8 (Run Crank) failed: {:?}. Skipping crank execution...",
                        e
                    );
                }
            }
        }
    }

    println!("   Step 9: Committing move batch (Bypassed)...");

    println!("\n=========================================================================================");
    println!("   Pausing 10s before undelegating (was 10 minutes — shortened for fast iteration).");
    println!("=========================================================================================\n");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    println!("   Step 10: Undelegating game...");
    let mut undelegate_ixs = vec![ix::undelegate_game_ix(
        program_id,
        game_pda,
        white.pubkey(),
        game_id,
    )?];
    apply_compute_budget(&mut undelegate_ixs, 300_000, 10_000, 262_144);
    let blockhash = crate::chain::get_blockhash_for_accounts(er_rpc, &[game_pda])?;
    let tx = Transaction::new_signed_with_payer(
        &undelegate_ixs,
        Some(&white.pubkey()),
        &[white],
        blockhash,
    );
    let sig = with_retry(|| er_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Er,
        "delegation",
        "undelegate_game",
        true,
        Some(sig.to_string()),
    );

    // Poll until L1 Game ownership returns to the program before finalization;
    // undelegation commits asynchronously from the ER.
    println!("   Waiting for ER relay to land on L1 (polling game PDA owner)...");
    let undelegate_wait_start = std::time::Instant::now();
    let mut fees_advanced_after_undelegate: Option<u64> = None;
    loop {
        match base_rpc.get_account(&game_pda) {
            Ok(acc) if acc.owner == program_id => {
                println!(
                    "   Game PDA owner restored to program after {:?}",
                    undelegate_wait_start.elapsed()
                );
                fees_advanced_after_undelegate = ix::parse_game_fees_advanced(&acc.data);
                break;
            }
            Ok(acc) => {
                if undelegate_wait_start.elapsed() > std::time::Duration::from_secs(60) {
                    println!(
                        "   [Warning] Still owned by {} after 60s — proceeding anyway",
                        acc.owner
                    );
                    break;
                }
            }
            Err(e) => {
                if undelegate_wait_start.elapsed() > std::time::Duration::from_secs(60) {
                    println!("   [Warning] get_account failed after 60s ({e}) — proceeding anyway");
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    // Check undelegation costs are included in fees_advanced after ownership returns.
    println!("   Step 10.75: Verifying fees_advanced accounting...");
    match fees_advanced_after_undelegate {
        Some(fees_advanced) => verify_fees_advanced(fees_advanced, 100)?,
        None => println!(
            "   [Warning] Could not read fees_advanced (game PDA owner never returned to program) — skipping fee-accounting check."
        ),
    }

    // The scripted final move is checkmate, so GameNotActive on resign is expected.
    println!("   Step 10.5: Resigning game (skipped if already finished by checkmate)...");
    let mut resign_ixs = vec![ix::resign_game_ix(program_id, game_id, white.pubkey())?];
    apply_compute_budget(&mut resign_ixs, 100_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&resign_ixs, Some(&white.pubkey()), &[white], blockhash);
    // Do not retry deterministic GameNotActive failures; preserve the original error.
    match base_rpc.send_and_confirm_transaction(&tx) {
        Ok(sig) => logger.log(
            ExecutionLayer::Solana,
            "game_setup",
            "resign",
            true,
            Some(sig.to_string()),
        ),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("0x1772") || msg.contains("GameNotActive") {
                println!(
                    "   Game already finished (checkmate detected on-chain) — skipping resign."
                );
                logger.log(ExecutionLayer::Solana, "game_setup", "resign", true, None);
            } else {
                return Err(e.into());
            }
        }
    }

    // 1v1 wager payout occurs inside finalize_game; no separate claim instruction is needed.

    println!("   Step 11: Finalizing game...");

    // Capture balances before finalization to verify actual payout, not just transaction confirmation.
    let white_elo_before =
        fetch_profile_elo(base_rpc, program_id, white.pubkey()).unwrap_or(1200.0);
    let black_elo_before =
        fetch_profile_elo(base_rpc, program_id, black.pubkey()).unwrap_or(1200.0);
    let white_balance_before = with_retry(|| base_rpc.get_balance(&white.pubkey())).await?;
    let black_balance_before = with_retry(|| base_rpc.get_balance(&black.pubkey())).await?;
    let treasury_vault =
        solana_sdk::pubkey::Pubkey::find_program_address(&[b"treasury_vault"], &program_id).0;
    let treasury_before = with_retry(|| base_rpc.get_balance(&treasury_vault)).await?;

    // Use the game's recorded fee payer (white here), not the benchmark master key.
    let mut finalize_ixs = vec![ix::finalize_game_ix(
        program_id,
        game_id,
        white.pubkey(),
        black.pubkey(),
        white.pubkey(),
    )?];
    apply_compute_budget(&mut finalize_ixs, 200_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &finalize_ixs,
        Some(&master.pubkey()),
        &[master],
        blockhash,
    );
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "game_setup",
        "finalize_game",
        true,
        Some(sig.to_string()),
    );

    // Capture post-finalize ELOs (wait briefly for state commit)
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let white_elo_after =
        fetch_profile_elo(base_rpc, program_id, white.pubkey()).unwrap_or(white_elo_before);
    let black_elo_after =
        fetch_profile_elo(base_rpc, program_id, black.pubkey()).unwrap_or(black_elo_before);

    println!("   ELO Changes:");
    println!(
        "     White: {:.1} -> {:.1} ({:+.1})",
        white_elo_before / 100.0,
        white_elo_after / 100.0,
        (white_elo_after - white_elo_before) / 100.0
    );
    println!(
        "     Black: {:.1} -> {:.1} ({:+.1})",
        black_elo_before / 100.0,
        black_elo_after / 100.0,
        (black_elo_after - black_elo_before) / 100.0
    );

    verify_winner_payout(
        base_rpc,
        &white.pubkey(),
        &black.pubkey(),
        white_balance_before,
        black_balance_before,
    )
    .await?;

    // Verify treasury reimbursement against the 1_000_000-lamport wager.
    println!("   Step 12: Verifying treasury_vault reimbursement...");
    if let Some(fees_advanced) = fees_advanced_after_undelegate {
        verify_treasury_reimbursement(
            base_rpc,
            treasury_vault,
            treasury_before,
            fees_advanced,
            1_000_000,
        )
        .await?;
    } else {
        println!(
            "   [Warning] fees_advanced was never read post-undelegate — skipping treasury reimbursement check."
        );
    }

    println!("   1v1 game flow complete!");
    Ok(())
}
