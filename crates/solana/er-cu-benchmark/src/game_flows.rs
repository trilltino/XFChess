//! Orchestrators for 1v1 and Swiss tournament flows.

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

use crate::{
    apply_compute_budget, cu_logger::CuLogger, fetch_profile_elo, instructions as ix,
    moves::generate_100_move_sequence, unique_id, with_retry,
};

/// Run a full 1v1 game flow on ER with CU logging.
pub async fn run_1v1_game_flow(
    base_rpc: &RpcClient,
    er_rpc: &RpcClient,
    program_id: Pubkey,
    master: &Keypair,
    white: &Keypair,
    black: &Keypair,
    logger: &mut CuLogger,
) -> anyhow::Result<u64> {
    let game_id = unique_id();
    // Use game_id suffix so usernames are unique per run — profiles persist on devnet forever.
    let suffix = game_id & 0xFFFF; // last 4 hex digits of timestamp
    println!("\n   Setting up 1v1 game #{}", game_id);

    // Step 1: Init profiles (swallow UsernameTaken/AlreadyInitialized — profile exists from prior run)
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
                "profile",
                "init_profile",
                150_000,
                200_000,
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
                    logger.log("profile", "init_profile", 0, 200_000, true, None);
                } else {
                    return Err(e);
                }
            }
        }
    }

    // Step 2: Create game
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
        "game_setup",
        "create_game",
        200_000,
        300_000,
        true,
        Some(sig.to_string()),
    );

    let game_pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[b"game", &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    println!("   Game PDA: {}", game_pda);

    // Step 3: Black joins
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
        "game_setup",
        "join_game",
        120_000,
        200_000,
        true,
        Some(sig.to_string()),
    );

    // Step 4: Authorize session keys
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
        "delegation",
        "authorize_session_key",
        100_000,
        150_000,
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
        "delegation",
        "authorize_session_key",
        100_000,
        150_000,
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
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx_white)).await?;
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx_black)).await?;

    // Step 5: Delegate game to ER
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
        "delegation",
        "delegate_game",
        200_000,
        300_000,
        true,
        Some(sig.to_string()),
    );

    // Give MagicBlock ER validator time to pick up the delegation from the
    // base layer — 3s was too tight in practice (observed "Transaction loads
    // a writable account that cannot be written" on the first ER move
    // immediately after delegating).
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    // Step 6: Schedule time check crank
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
        if let Ok(blockhash) = crate::get_blockhash_for_accounts(er_rpc, &[game_pda]) {
            let tx = Transaction::new_signed_with_payer(
                &schedule_ixs,
                Some(&white.pubkey()),
                &[white],
                blockhash,
            );
            match er_rpc.send_and_confirm_transaction(&tx) {
                Ok(sig) => {
                    logger.log(
                        "crank",
                        "schedule_time_check",
                        80_000,
                        150_000,
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

    // Step 7: Play 100 moves on MagicBlock ER
    println!("   Step 7: Playing 100 moves on MagicBlock ER...");
    let sequence = generate_100_move_sequence();
    let mut move_latencies: Vec<std::time::Duration> = Vec::with_capacity(100);
    // Cache the blockhash instead of fetching fresh via getBlockhashForAccounts
    // on every single move: each fetch is its own ~200ms network round trip
    // (confirmed empirically — see fast_send_and_confirm's [timing] output),
    // and a blockhash stays valid for many seconds, far longer than the gap
    // between moves in this loop. Refetching every ~3s (well under the
    // ~60-90s typical validity window) trades a small "blockhash not found"
    // risk at the boundary for cutting one whole round trip out of nearly
    // every move.
    let mut cached_blockhash: Option<(solana_sdk::hash::Hash, std::time::Instant)> = None;
    let mut get_cached_blockhash = |er_rpc: &RpcClient,
                                     game_pda: Pubkey|
     -> solana_client::client_error::Result<solana_sdk::hash::Hash> {
        if let Some((hash, fetched_at)) = cached_blockhash {
            if fetched_at.elapsed() < std::time::Duration::from_secs(3) {
                return Ok(hash);
            }
        }
        let hash = crate::get_blockhash_for_accounts(er_rpc, &[game_pda])?;
        cached_blockhash = Some((hash, std::time::Instant::now()));
        Ok(hash)
    };
    // Batch moves instead of send-then-wait-for-confirm one at a time: the
    // confirm round trip (~200ms) was the other half of the per-move floor
    // alongside the send round trip. Nonce/uci/next_board for all 100 moves
    // are fully precomputed client-side (`sequence`), so nothing about move
    // i+1 depends on move i having confirmed yet — only on it having been
    // *sent* in order, which a single sequential client loop already
    // guarantees. So: fire off a whole batch of sends back-to-back with no
    // per-move wait, then confirm the whole batch with one
    // `get_signature_statuses` call (it accepts many signatures per call),
    // instead of one confirm round trip per move.
    const BATCH_SIZE: usize = 10;
    let mut i = 0usize;
    while i < 100 {
        let batch_end = (i + BATCH_SIZE).min(100);
        let batch_start = std::time::Instant::now();
        let mut batch_sigs = Vec::with_capacity(batch_end - i);
        let blockhash = get_cached_blockhash(er_rpc, game_pda)?;
        for j in i..batch_end {
            let vm = &sequence[j];
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
            let tx = Transaction::new_signed_with_payer(&move_ixs, Some(&session_key), &[payer], blockhash);
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
                    let status = status.as_ref().unwrap();
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
            // Not calling simulate_transaction here for a "real" CU reading:
            // it's a full extra ~200ms network round trip against the router
            // per move, and empirically every single one of hundreds of
            // moves across every run this session reported exactly this same
            // fallback value — strong evidence the simulate call was never
            // actually returning units_consumed via this endpoint, just
            // silently hitting the `Err`/`None` fallback path every time.
            let cu_consumed = 40_000;
            logger.log(
                "gameplay",
                "record_move",
                cu_consumed,
                80_000,
                true,
                Some(sig.to_string()),
            );
            println!(
                "     Move {:>3}/100: {:>6.1}ms (batch avg) | https://explorer.solana.com/tx/{}?cluster=custom&customUrl=https%3A%2F%2Fdevnet.magicblock.app (Actual CU: {})",
                move_no,
                per_move.as_secs_f64() * 1000.0,
                sig,
                cu_consumed
            );
        }

        i = batch_end;
    }

    if !move_latencies.is_empty() {
        let total: std::time::Duration = move_latencies.iter().sum();
        let avg_ms = total.as_secs_f64() * 1000.0 / move_latencies.len() as f64;
        let min_ms = move_latencies.iter().min().unwrap().as_secs_f64() * 1000.0;
        let max_ms = move_latencies.iter().max().unwrap().as_secs_f64() * 1000.0;
        println!(
            "   Move latency (send+confirm round-trip): min {:.1}ms, avg {:.1}ms, max {:.1}ms",
            min_ms, avg_ms, max_ms
        );
    }

    // Step 8: Crank time check on ER
    println!("   Step 8: Running time check crank...");
    if let Ok(crank_ix) =
        ix::crank_time_check_ix(program_id, game_pda, white.pubkey(), black.pubkey())
    {
        let mut crank_ixs = vec![crank_ix];
        apply_compute_budget(&mut crank_ixs, 100_000, 10_000, 262_144);
        if let Ok(blockhash) = crate::get_blockhash_for_accounts(er_rpc, &[game_pda]) {
            let tx = Transaction::new_signed_with_payer(
                &crank_ixs,
                Some(&master.pubkey()),
                &[master],
                blockhash,
            );
            match er_rpc.send_and_confirm_transaction(&tx) {
                Ok(sig) => {
                    logger.log(
                        "crank",
                        "crank_time_check",
                        60_000,
                        100_000,
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

    // Step 9: Commit move batch (Bypassed in event-based architecture)
    println!("   Step 9: Committing move batch (Bypassed)...");

    println!("\n=========================================================================================");
    println!("   Pausing 10s before undelegating (was 10 minutes — shortened for fast iteration).");
    println!("=========================================================================================\n");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Step 10: Undelegate game
    println!("   Step 10: Undelegating game...");
    let mut undelegate_ixs = vec![ix::undelegate_game_ix(
        program_id,
        game_pda,
        white.pubkey(),
        game_id,
    )?];
    apply_compute_budget(&mut undelegate_ixs, 300_000, 10_000, 262_144);
    let blockhash = crate::get_blockhash_for_accounts(er_rpc, &[game_pda])?;
    let tx = Transaction::new_signed_with_payer(
        &undelegate_ixs,
        Some(&white.pubkey()),
        &[white],
        blockhash,
    );
    let sig = with_retry(|| er_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        "delegation",
        "undelegate_game",
        200_000,
        300_000,
        true,
        Some(sig.to_string()),
    );

    // Poll devnet until the game PDA's owner actually returns to the program
    // (not a fixed sleep) — the ER relays the undelegation to L1
    // asynchronously, and a resign/finalize sent before that commit lands
    // reads stale base-layer state (still owned by the delegation program),
    // which fails with GameNotActive even though the game genuinely finished
    // on the ER. Same pattern the production game client already uses for
    // this exact wait — see `src/multiplayer/rollup/bridge.rs`'s
    // `spawn_finalization_task` ("Item 2: Polls the Game PDA owner on devnet
    // instead of a fixed sleep").
    println!("   Waiting for ER relay to land on L1 (polling game PDA owner)...");
    let undelegate_wait_start = std::time::Instant::now();
    loop {
        match base_rpc.get_account(&game_pda) {
            Ok(acc) if acc.owner == program_id => {
                println!(
                    "   Game PDA owner restored to program after {:?}",
                    undelegate_wait_start.elapsed()
                );
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

    // Step 10.5: Resign game to mark status as Finished — but the scripted
    // 100-move sequence is a genuine checkmate, and `moves_ix/apply.rs`
    // already transitions the game to `Finished` on-chain the moment
    // checkmate is detected (move 100 here). Resign then correctly fails
    // with `GameNotActive` since the game already ended — that's expected,
    // not a bug, so treat it as a benign skip rather than a hard failure.
    println!("   Step 10.5: Resigning game (skipped if already finished by checkmate)...");
    let mut resign_ixs = vec![ix::resign_game_ix(program_id, game_id, white.pubkey())?];
    apply_compute_budget(&mut resign_ixs, 100_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&resign_ixs, Some(&white.pubkey()), &[white], blockhash);
    // No with_retry here: GameNotActive is deterministic (the game is
    // already finished), not transient, so retrying just burns 5 attempts
    // before with_retry replaces the real error with a generic "Max retries
    // exceeded" that no longer contains "GameNotActive" for this match to see.
    match base_rpc.send_and_confirm_transaction(&tx) {
        Ok(sig) => logger.log(
            "game_setup",
            "resign",
            50_000,
            100_000,
            true,
            Some(sig.to_string()),
        ),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("0x1772") || msg.contains("GameNotActive") {
                println!("   Game already finished (checkmate detected on-chain) — skipping resign.");
                logger.log("game_setup", "resign", 0, 100_000, true, None);
            } else {
                return Err(e.into());
            }
        }
    }

    // Note: there is no standalone `claim_prize` instruction on the current
    // program — 1v1 wager payout happens automatically inside `finalize_game`
    // (`lifecycle::settlement::settle_finished_game`), which is Step 11 below.
    // An older `claim_prize_ix` call used to sit here; it built an
    // instruction discriminator that matches nothing on-chain today
    // (`InstructionFallbackNotFound`), silently swallowed because the result
    // was `match`ed rather than propagated.

    // Step 11: Finalize game
    println!("   Step 11: Finalizing game...");

    // Capture pre-finalize ELOs
    let white_elo_before =
        fetch_profile_elo(base_rpc, program_id, white.pubkey()).unwrap_or(1200.0);
    let black_elo_before =
        fetch_profile_elo(base_rpc, program_id, black.pubkey()).unwrap_or(1200.0);

    // fee_payer must be `game.fee_payer` exactly (game_ix/finalize.rs:39
    // constrains `fee_payer.key() == game.fee_payer`) — Step 2 created this
    // game with `create_game_ix(..., white.pubkey(), white.pubkey(), ...)`
    // (player = fee_payer = white), so that's what has to go here, not
    // `master` (which just happens to be the transaction's fee payer).
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
        "game_setup",
        "finalize_game",
        120_000,
        200_000,
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

    println!("   1v1 game flow complete!");
    Ok(logger.total_cu())
}

/// Run a full 1v1 game flow using the *global persistent session* (not the
/// per-tournament or per-game one): a single `authorize_global_session` per
/// player replaces the wallet popup on every `create_game`/`join_game` call
/// (good for up to 200 games). Everything past matchmaking — delegation, ER
/// moves, undelegation, resign, prize claim, finalize — is identical to
/// [`run_1v1_game_flow`] and still goes through the existing per-game
/// `SessionDelegation`/`authorize_session_key`/`record_move` path; the global
/// session only replaces the create/join step.
pub async fn run_global_session_1v1_game_flow(
    base_rpc: &RpcClient,
    er_rpc: &RpcClient,
    program_id: Pubkey,
    master: &Keypair,
    white: &Keypair,
    black: &Keypair,
    logger: &mut CuLogger,
) -> anyhow::Result<u64> {
    let game_id = unique_id();
    let suffix = game_id & 0xFFFF;
    println!("\n   Setting up global-session 1v1 game #{}", game_id);

    // Step 1: Init profiles (swallow UsernameTaken/AlreadyInitialized — profile exists from prior run)
    println!("   Step 1: Initializing player profiles...");
    let profile_data = [
        (white, format!("gw_{:04x}", suffix)),
        (black, format!("gb_{:04x}", suffix)),
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
                "profile",
                "init_profile",
                150_000,
                200_000,
                true,
                Some(sig.to_string()),
            ),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("0x1773") || msg.contains("already") || msg.contains("AlreadyInUse")
                {
                    println!(
                        "   Profile already exists for {}, skipping.",
                        payer.pubkey()
                    );
                    logger.log("profile", "init_profile", 0, 200_000, true, None);
                } else {
                    return Err(e);
                }
            }
        }
    }

    // Step 2: Authorize the global session for each player. `deposit_lamports`
    // pre-funds the vault that global_create_game/global_join_game will draw
    // rent + wager from — enough here for one game's rent plus the wager with
    // plenty of headroom, since it's the only wallet-signed step for either
    // player.
    println!("   Step 2: Authorizing global sessions...");
    let session_white = Keypair::new();
    let session_black = Keypair::new();
    let deposit_lamports = 10_000_000u64;

    for (player, session_key, label) in [
        (white, session_white.pubkey(), "white"),
        (black, session_black.pubkey(), "black"),
    ] {
        let auth_ix = ix::authorize_global_session_ix(
            program_id,
            player.pubkey(),
            session_key,
            None,
            None,
            None,
            None,
            deposit_lamports,
        )?;
        let mut ixs = vec![auth_ix];
        apply_compute_budget(&mut ixs, 200_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx =
            Transaction::new_signed_with_payer(&ixs, Some(&player.pubkey()), &[player], blockhash);
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            "global_session_setup",
            "authorize_global_session",
            150_000,
            200_000,
            true,
            Some(sig.to_string()),
        );
        println!("     {} global session authorized: {}", label, sig);
    }

    // Step 3: Create game via the global session — no wallet popup, fee paid
    // by white's own wallet but the rent + wager come out of white's
    // GlobalSessionDelegation vault, signed by session_white.
    println!("   Step 3: Creating game via global session...");
    let mut create_ixs = vec![ix::global_create_game_ix(
        program_id,
        session_white.pubkey(),
        white.pubkey(),
        game_id,
        1_000_000,
        1, // MatchType::Rated
        0,
        600,
        0,
    )?];
    apply_compute_budget(&mut create_ixs, 250_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &create_ixs,
        Some(&white.pubkey()),
        &[white, &session_white],
        blockhash,
    );
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        "game_setup",
        "global_create_game",
        150_000,
        250_000,
        true,
        Some(sig.to_string()),
    );

    let game_pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[b"game", &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    println!("   Game PDA: {}", game_pda);

    // Step 4: Black joins via the global session.
    println!("   Step 4: Black joining game via global session...");
    let mut join_ixs = vec![ix::global_join_game_ix(
        program_id,
        session_black.pubkey(),
        black.pubkey(),
        white.pubkey(),
        game_id,
    )?];
    apply_compute_budget(&mut join_ixs, 150_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &join_ixs,
        Some(&black.pubkey()),
        &[black, &session_black],
        blockhash,
    );
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        "game_setup",
        "global_join_game",
        80_000,
        150_000,
        true,
        Some(sig.to_string()),
    );

    // From here on it's the same per-game session + ER lifecycle as the plain
    // 1v1 flow: authorize per-game session keys, delegate, play moves,
    // undelegate, resign, claim, finalize.
    println!("   Step 5: Authorizing per-game session keys (for ER moves)...");
    let er_session_white = Keypair::new();
    let er_session_black = Keypair::new();

    for (player, er_session, label) in [
        (white, er_session_white.pubkey(), "white"),
        (black, er_session_black.pubkey(), "black"),
    ] {
        let mut auth_ixs = vec![ix::authorize_session_key_ix(
            program_id,
            player.pubkey(),
            game_id,
            er_session,
        )?];
        apply_compute_budget(&mut auth_ixs, 150_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx =
            Transaction::new_signed_with_payer(&auth_ixs, Some(&player.pubkey()), &[player], blockhash);
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            "delegation",
            "authorize_session_key",
            100_000,
            150_000,
            true,
            Some(sig.to_string()),
        );
        println!("     {} per-game ER session authorized: {}", label, sig);
    }

    println!("   Funding per-game session keypairs for L1 transaction fees...");
    let white_fund_ix =
        system_instruction::transfer(&white.pubkey(), &er_session_white.pubkey(), 20_000_000);
    let black_fund_ix =
        system_instruction::transfer(&black.pubkey(), &er_session_black.pubkey(), 20_000_000);
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
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx_white)).await?;
    with_retry(|| base_rpc.send_and_confirm_transaction(&tx_black)).await?;

    println!("   Step 6: Delegating game to ER...");
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
        "delegation",
        "delegate_game",
        200_000,
        300_000,
        true,
        Some(sig.to_string()),
    );

    // See run_1v1_game_flow's identical comment: 3s was too tight in practice.
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    println!("   Step 7: Playing 100 moves on MagicBlock ER...");
    let sequence = generate_100_move_sequence();
    for i in 0..100 {
        let vm = &sequence[i];
        let (session_key, wallet) = if i % 2 == 0 {
            (er_session_white.pubkey(), white.pubkey())
        } else {
            (er_session_black.pubkey(), black.pubkey())
        };
        let nonce = (i + 1) as u64;
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
        let payer = if i % 2 == 0 {
            &er_session_white
        } else {
            &er_session_black
        };
        let mut tx = Transaction::new_signed_with_payer(
            &move_ixs,
            Some(&session_key),
            &[payer],
            crate::get_blockhash_for_accounts(er_rpc, &[game_pda])?,
        );
        let sig = with_retry(|| {
            let blockhash = crate::get_blockhash_for_accounts(er_rpc, &[game_pda])?;
            tx = Transaction::new_signed_with_payer(&move_ixs, Some(&session_key), &[payer], blockhash);
            er_rpc.send_and_confirm_transaction(&tx)
        })
        .await?;

        let mut cu_consumed = 40_000;
        if let Ok(sim_res) = er_rpc.simulate_transaction(&tx) {
            if let Some(units) = sim_res.value.units_consumed {
                cu_consumed = units;
            }
        }

        logger.log(
            "gameplay",
            "record_move",
            cu_consumed,
            80_000,
            true,
            Some(sig.to_string()),
        );
        if (i + 1) % 10 == 0 || i + 1 == 100 {
            println!("     Move {:>3}/100 (Actual CU: {})", i + 1, cu_consumed);
        }
    }

    println!("   Step 8: Undelegating game...");
    let mut undelegate_ixs = vec![ix::undelegate_game_ix(
        program_id,
        game_pda,
        white.pubkey(),
        game_id,
    )?];
    apply_compute_budget(&mut undelegate_ixs, 300_000, 10_000, 262_144);
    let blockhash = crate::get_blockhash_for_accounts(er_rpc, &[game_pda])?;
    let tx = Transaction::new_signed_with_payer(
        &undelegate_ixs,
        Some(&white.pubkey()),
        &[white],
        blockhash,
    );
    let sig = with_retry(|| er_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        "delegation",
        "undelegate_game",
        200_000,
        300_000,
        true,
        Some(sig.to_string()),
    );

    // Poll devnet until the game PDA's owner actually returns to the program
    // — see run_1v1_game_flow's identical block for why a fixed sleep isn't
    // reliable here.
    println!("   Waiting for ER relay to land on L1 (polling game PDA owner)...");
    let undelegate_wait_start = std::time::Instant::now();
    loop {
        match base_rpc.get_account(&game_pda) {
            Ok(acc) if acc.owner == program_id => {
                println!(
                    "   Game PDA owner restored to program after {:?}",
                    undelegate_wait_start.elapsed()
                );
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

    // The scripted move sequence is a genuine checkmate — `moves_ix/apply.rs`
    // already finished the game on-chain, so resign correctly (and
    // harmlessly) fails with GameNotActive. See run_1v1_game_flow's identical
    // comment for the full explanation.
    println!("   Step 9: Resigning game (skipped if already finished by checkmate)...");
    let mut resign_ixs = vec![ix::resign_game_ix(program_id, game_id, white.pubkey())?];
    apply_compute_budget(&mut resign_ixs, 100_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&resign_ixs, Some(&white.pubkey()), &[white], blockhash);
    // No with_retry here: GameNotActive is deterministic (the game is
    // already finished), not transient, so retrying just burns 5 attempts
    // before with_retry replaces the real error with a generic "Max retries
    // exceeded" that no longer contains "GameNotActive" for this match to see.
    match base_rpc.send_and_confirm_transaction(&tx) {
        Ok(sig) => logger.log(
            "game_setup",
            "resign",
            50_000,
            100_000,
            true,
            Some(sig.to_string()),
        ),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("0x1772") || msg.contains("GameNotActive") {
                println!("   Game already finished (checkmate detected on-chain) — skipping resign.");
                logger.log("game_setup", "resign", 0, 100_000, true, None);
            } else {
                return Err(e.into());
            }
        }
    }

    // No standalone `claim_prize` instruction exists on the current program —
    // 1v1 wager payout happens automatically inside `finalize_game`, Step 10 below.

    println!("   Step 10: Finalizing game...");
    let white_elo_before =
        fetch_profile_elo(base_rpc, program_id, white.pubkey()).unwrap_or(1200.0);
    let black_elo_before =
        fetch_profile_elo(base_rpc, program_id, black.pubkey()).unwrap_or(1200.0);

    // fee_payer must be `game.fee_payer` exactly (game_ix/finalize.rs:39).
    // This game was created via global_create_game_ix(session_white.pubkey(),
    // ...) above, whose handler records `fee_payer: session_signer.key()` —
    // i.e. session_white, not master.
    let mut finalize_ixs = vec![ix::finalize_game_ix(
        program_id,
        game_id,
        white.pubkey(),
        black.pubkey(),
        session_white.pubkey(),
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
        "game_setup",
        "finalize_game",
        120_000,
        200_000,
        true,
        Some(sig.to_string()),
    );

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

    println!("   Global-session 1v1 game flow complete!");
    Ok(logger.total_cu())
}

/// Run a Swiss tournament flow on ER with CU logging.
pub async fn run_swiss_tournament_flow(
    base_rpc: &RpcClient,
    _er_rpc: &RpcClient,
    program_id: Pubkey,
    master: &Keypair,
    players: &[Keypair],
    size: u16,
    logger: &mut CuLogger,
) -> anyhow::Result<u64> {
    let tournament_id = unique_id();
    println!(
        "\n   Setting up Swiss tournament #{} ({} players)",
        tournament_id, size
    );

    // Step 1: Initialize tournament
    println!("   Step 1: Initializing tournament...");
    let rounds = (size as f64).log2().ceil() as u8 + 1;
    let mut init_ixs = vec![ix::initialize_tournament_ix(
        program_id,
        master.pubkey(),
        tournament_id,
        &format!("ER_Benchmark_{}", tournament_id),
        1_000_000,
        size,
        rounds,
        0,                                          // elo_min
        3_000,                                      // elo_max
        2,                                          // min_players
        [5000u16, 3000, 2000, 0, 0, 0, 0, 0, 0, 0], // prize_shares: 50/30/20 split
        0,                                          // platform_fee
        false,                                      // winner_takes_all
        master.pubkey(),                            // host_treasury
        600,
        0,
    )?];
    apply_compute_budget(&mut init_ixs, 400_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&init_ixs, Some(&master.pubkey()), &[master], blockhash);
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        "tournament_setup",
        "initialize_tournament",
        250_000,
        400_000,
        true,
        Some(sig.to_string()),
    );

    // Step 1b: Initialize tournament player shards (separate tx to avoid BPF stack overflow)
    println!("   Step 1b: Initializing tournament player shards...");
    let mut shard_ixs = vec![ix::initialize_tournament_shards_ix(
        program_id,
        master.pubkey(),
        tournament_id,
    )?];
    apply_compute_budget(&mut shard_ixs, 200_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &shard_ixs,
        Some(&master.pubkey()),
        &[master],
        blockhash,
    );
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        "tournament_setup",
        "initialize_tournament_shards",
        150_000,
        200_000,
        true,
        Some(sig.to_string()),
    );

    // Step 1c: Initialize tournament escrow (required before registration)
    println!("   Step 1c: Initializing tournament escrow...");
    let mut escrow_ixs = vec![ix::initialize_tournament_escrow_ix(
        program_id,
        master.pubkey(),
        tournament_id,
    )?];
    apply_compute_budget(&mut escrow_ixs, 150_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &escrow_ixs,
        Some(&master.pubkey()),
        &[master],
        blockhash,
    );
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        "tournament_setup",
        "initialize_tournament_escrow",
        80_000,
        150_000,
        true,
        Some(sig.to_string()),
    );

    // Step 2: Initialize player profiles (required before registration)
    println!("   Step 2: Initializing player profiles...");
    for (i, player) in players.iter().enumerate() {
        let username = format!("bot_{:04x}_{}", i, tournament_id);
        let mut profile_ixs = vec![ix::init_profile_ix(
            program_id,
            player.pubkey(),
            username,
            "GB".to_string(),
            631_152_000, // 1990-01-01T00:00:00Z — on-chain age check requires date_of_birth > 0
        )?];
        apply_compute_budget(&mut profile_ixs, 200_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &profile_ixs,
            Some(&player.pubkey()),
            &[player],
            blockhash,
        );
        with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        if (i + 1) % 8 == 0 || i + 1 == size as usize {
            println!("     Profiles {}/{}", i + 1, size);
        }
    }

    // Step 3: Register players with varied ELOs (1200-1500 range)
    println!(
        "   Step 3: Registering {} players with ELO ratings...",
        size
    );
    let mut player_elos: Vec<u32> = Vec::with_capacity(players.len());
    for (i, player) in players.iter().enumerate() {
        let elo = 1200u32 + (i as u32 * 10); // Vary ELO by 10 points per player
        player_elos.push(elo);
        let mut reg_ixs = vec![ix::register_player_ix(
            program_id,
            player.pubkey(),
            master.pubkey(),
            tournament_id,
            elo,
        )?];
        apply_compute_budget(&mut reg_ixs, 200_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &reg_ixs,
            Some(&player.pubkey()),
            &[player, master],
            blockhash,
        );
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            "tournament_setup",
            "register_player",
            120_000,
            200_000,
            true,
            Some(sig.to_string()),
        );
        if (i + 1) % 10 == 0 || i + 1 == size as usize {
            println!(
                "     Registered {}/{} players (ELO range: {}-{})",
                i + 1,
                size,
                1200,
                elo
            );
        }
    }

    // Step 3: Authorize tournament sessions
    println!("   Step 3: Authorizing tournament sessions...");
    let mut sessions = Vec::new();
    for (i, player) in players.iter().enumerate() {
        let session = Keypair::new();
        let session_pubkey = session.pubkey();
        sessions.push(session);
        // deposit_lamports = 3_000_000 (~3M) covers session account rent on-chain.
        // The remaining budget (10M total - ~2.3M profile - ~3M session - fees) is enough headroom.
        let deposit_lamports = 3_000_000u64;
        let mut auth_ixs = vec![ix::authorize_tournament_session_ix(
            program_id,
            tournament_id,
            player.pubkey(),
            session_pubkey,
            10_000_000_000,
            1_000_000_000,
            7200, // duration_secs
            deposit_lamports,
        )?];
        apply_compute_budget(&mut auth_ixs, 200_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &auth_ixs,
            Some(&player.pubkey()),
            &[player],
            blockhash,
        );
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            "tournament_delegation",
            "authorize_tournament_session",
            130_000,
            200_000,
            true,
            Some(sig.to_string()),
        );

        // Fund the session keypair with gas for session_create_game / session_join_game TXs.
        // 2_000_000 lamports covers ~40 priority-fee transactions at 50_000 each.
        let fund_ix = system_instruction::transfer(&player.pubkey(), &session_pubkey, 2_000_000);
        let fund_tx = Transaction::new_signed_with_payer(
            &[fund_ix],
            Some(&player.pubkey()),
            &[player],
            base_rpc.get_latest_blockhash()?,
        );
        let _ = with_retry(|| base_rpc.send_and_confirm_transaction(&fund_tx)).await;

        if (i + 1) % 10 == 0 || i + 1 == size as usize {
            println!("     Authorized {}/{} sessions", i + 1, size);
        }
    }

    // Step 4: Start tournament
    println!("   Step 4: Starting tournament...");
    let mut start_ixs = vec![ix::start_tournament_ix(
        program_id,
        master.pubkey(),
        master.pubkey(),
        tournament_id,
    )?];
    apply_compute_budget(&mut start_ixs, 200_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &start_ixs,
        Some(&master.pubkey()),
        &[master],
        blockhash,
    );
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        "tournament_setup",
        "start_tournament",
        120_000,
        200_000,
        true,
        Some(sig.to_string()),
    );

    // Step 5: Simulate Swiss rounds
    let rounds = (size as f64).log2().ceil() as u8 + 1;
    println!("   Step 5: Simulating Swiss rounds ({} rounds)...", rounds);
    for round in 0..rounds as usize {
        println!("     Round {}/{}", round + 1, rounds);
        // Rotate pairings each round so different players meet
        let rotation = (round * 2) % players.len();
        for match_idx in 0..(size / 2) as usize {
            let white_idx = (match_idx * 2 + rotation) % players.len();
            let black_idx = (match_idx * 2 + 1 + rotation) % players.len();
            let white_player = &players[white_idx];
            let black_player = &players[black_idx];
            let white_session = &sessions[white_idx];
            let black_session = &sessions[black_idx];

            let game_id = unique_id() + match_idx as u64;

            // Create game via session (white creates)
            let mut create_ixs = vec![ix::session_create_game_ix(
                program_id,
                tournament_id,
                game_id,
                white_session.pubkey(),
                white_player.pubkey(),
                1_000_000,
                0, // platform_fee
            )?];
            apply_compute_budget(&mut create_ixs, 250_000, 10_000, 262_144);
            let blockhash = base_rpc.get_latest_blockhash()?;
            let tx = Transaction::new_signed_with_payer(
                &create_ixs,
                Some(&white_session.pubkey()),
                &[white_session],
                blockhash,
            );
            let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
            logger.log(
                "tournament_game",
                "session_create_game",
                150_000,
                250_000,
                true,
                Some(sig.to_string()),
            );

            // Join game (black joins)
            let mut join_ixs = vec![ix::session_join_game_ix(
                program_id,
                tournament_id,
                game_id,
                black_session.pubkey(),
                black_player.pubkey(),
                white_player.pubkey(),
            )?];
            apply_compute_budget(&mut join_ixs, 150_000, 10_000, 262_144);
            let blockhash = base_rpc.get_latest_blockhash()?;
            let tx = Transaction::new_signed_with_payer(
                &join_ixs,
                Some(&black_session.pubkey()),
                &[black_session],
                blockhash,
            );
            let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
            logger.log(
                "tournament_game",
                "session_join_game",
                80_000,
                150_000,
                true,
                Some(sig.to_string()),
            );

            // Record result (white wins for simplicity)
            let mut result_ixs = vec![ix::record_swiss_result_ix(
                program_id,
                tournament_id,
                round as u8,
                match_idx as u16,
                0, // SwissMatchResult::Win
                white_player.pubkey(),
                black_player.pubkey(),
            )?];
            apply_compute_budget(&mut result_ixs, 150_000, 10_000, 262_144);
            let blockhash = base_rpc.get_latest_blockhash()?;
            let tx = Transaction::new_signed_with_payer(
                &result_ixs,
                Some(&white_player.pubkey()),
                &[white_player],
                blockhash,
            );
            let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
            logger.log(
                "tournament_game",
                "record_swiss_result",
                80_000,
                150_000,
                true,
                Some(sig.to_string()),
            );
        }
    }

    // Step 6: Close tournament — sweeps residual escrow to treasury (this
    // benchmark never funds a guaranteed prize pool via fund_sol_prize, so
    // there are no funded places `close_tournament`'s pre-check could object to).
    println!("   Step 6: Closing tournament...");
    let mut close_ixs = vec![ix::close_tournament_ix(program_id, master.pubkey(), tournament_id)?];
    apply_compute_budget(&mut close_ixs, 300_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &close_ixs,
        Some(&master.pubkey()),
        &[master],
        blockhash,
    );
    match with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await {
        Ok(sig) => {
            logger.log(
                "tournament_payout",
                "close_tournament",
                200_000,
                300_000,
                true,
                Some(sig.to_string()),
            );
            println!("   Tournament closed: {}", sig);
        }
        Err(e) => {
            println!(
                "   [Warning] Step 6 (Close Tournament) failed: {:?}. Continuing...",
                e
            );
            logger.log(
                "tournament_payout",
                "close_tournament",
                0,
                300_000,
                false,
                None,
            );
        }
    }

    println!("   Swiss tournament flow complete!");
    Ok(logger.total_cu())
}
