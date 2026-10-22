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

pub async fn run_global_session_1v1_game_flow(
    base_rpc: &RpcClient,
    er_rpc: &RpcClient,
    program_id: Pubkey,
    master: &Keypair,
    white: &Keypair,
    black: &Keypair,
    logger: &mut MeasurementLog,
) -> anyhow::Result<()> {
    let game_id = unique_id()?;
    let suffix = game_id & 0xFFFF;
    println!("\n   Setting up global-session 1v1 game #{}", game_id);

    // Existing profiles may return UsernameTaken or AlreadyInitialized on repeated runs.
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
                ExecutionLayer::Solana,
                "profile",
                "init_profile",
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

    // Prefund each global-session vault for game rent and wager during authorization.
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
            ExecutionLayer::Solana,
            "global_session_setup",
            "authorize_global_session",
            true,
            Some(sig.to_string()),
        );
        println!("     {} global session authorized: {}", label, sig);
    }

    // The wallet pays fees; the session-signed delegation vault fronts rent and wager.
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
        ExecutionLayer::Solana,
        "game_setup",
        "global_create_game",
        true,
        Some(sig.to_string()),
    );

    let game_pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[b"game", &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    println!("   Game PDA: {}", game_pda);

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
        ExecutionLayer::Solana,
        "game_setup",
        "global_join_game",
        true,
        Some(sig.to_string()),
    );

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
        let tx = Transaction::new_signed_with_payer(
            &auth_ixs,
            Some(&player.pubkey()),
            &[player],
            blockhash,
        );
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            ExecutionLayer::Solana,
            "delegation",
            "authorize_session_key",
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
        ExecutionLayer::Solana,
        "delegation",
        "delegate_game",
        true,
        Some(sig.to_string()),
    );

    // See run_1v1_game_flow's identical comment: 3s was too tight in practice.
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    println!("   Step 7: Playing 100 moves on MagicBlock ER...");
    let sequence = generate_100_move_sequence()?;
    for (i, vm) in sequence.iter().enumerate().take(100) {
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
            crate::chain::get_blockhash_for_accounts(er_rpc, &[game_pda])?,
        );
        let sig = with_retry(|| {
            let blockhash = crate::chain::get_blockhash_for_accounts(er_rpc, &[game_pda])?;
            tx = Transaction::new_signed_with_payer(
                &move_ixs,
                Some(&session_key),
                &[payer],
                blockhash,
            );
            er_rpc.send_and_confirm_transaction(&tx)
        })
        .await?;

        logger.log(
            ExecutionLayer::Er,
            "gameplay",
            "record_move",
            true,
            Some(sig.to_string()),
        );
        if (i + 1) % 10 == 0 || i + 1 == 100 {
            println!("     Move {:>3}/100", i + 1);
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

    // Poll until the Game PDA returns to program ownership; a fixed sleep is unreliable.
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

    // Checkmate may have ended the game already; GameNotActive on resign is benign.
    println!("   Step 9: Resigning game (skipped if already finished by checkmate)...");
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

    // finalize_game pays 1v1 wagers automatically; no separate claim_prize instruction exists.

    println!("   Step 10: Finalizing game...");
    let white_elo_before =
        fetch_profile_elo(base_rpc, program_id, white.pubkey()).unwrap_or(1200.0);
    let black_elo_before =
        fetch_profile_elo(base_rpc, program_id, black.pubkey()).unwrap_or(1200.0);
    let white_balance_before = with_retry(|| base_rpc.get_balance(&white.pubkey())).await?;
    let black_balance_before = with_retry(|| base_rpc.get_balance(&black.pubkey())).await?;

    // Use session_white as fee payer to match the game session, not the master wallet.
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
        ExecutionLayer::Solana,
        "game_setup",
        "finalize_game",
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

    verify_winner_payout(
        base_rpc,
        &white.pubkey(),
        &black.pubkey(),
        white_balance_before,
        black_balance_before,
    )
    .await?;

    println!("   Global-session 1v1 game flow complete!");
    Ok(())
}
