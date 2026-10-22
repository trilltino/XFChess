use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::chain::{
    apply_compute_budget, instructions as ix, unique_id, with_retry, TOURNAMENT_STATUS_COMPLETED,
};
use crate::cost::measurements::{ExecutionLayer, MeasurementLog};

use super::common::*;

pub async fn run_single_elimination_tournament_flow(
    base_rpc: &RpcClient,
    program_id: Pubkey,
    master: &Keypair,
    players: &[Keypair],
    size: u16,
    logger: &mut MeasurementLog,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        size.is_power_of_two() && (2..=64).contains(&size),
        "size must be a power of 2 between 2 and 64, got {size}"
    );
    anyhow::ensure!(
        players.len() == size as usize,
        "expected exactly {size} players, got {}",
        players.len()
    );

    let tournament_id = unique_id()?;
    println!(
        "\n   Setting up single-elimination tournament #{} ({} players)",
        tournament_id, size
    );
    // Mirrors state::get_default_prize_shares's low-size branches (this
    // benchmark crate doesn't depend on the on-chain program's Rust types).
    let prize_shares: [u16; 10] = if size <= 2 {
        [7000, 3000, 0, 0, 0, 0, 0, 0, 0, 0]
    } else {
        [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0]
    };

    println!("   Step 1: Initializing tournament...");
    let mut init_ixs = vec![ix::initialize_tournament_ix(
        program_id,
        master.pubkey(),
        tournament_id,
        &format!("ER_Benchmark_SE_{}", tournament_id),
        1_000_000,
        size,
        None,
        0,     // elo_min
        3_000, // elo_max
        2,     // min_players
        prize_shares,
        0,               // platform_fee
        false,           // winner_takes_all
        master.pubkey(), // host_treasury
        60, // 1 min base clock — synthetic instant-result benchmark, no real clock needed
        0,
    )?];
    apply_compute_budget(&mut init_ixs, 400_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&init_ixs, Some(&master.pubkey()), &[master], blockhash);
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "tournament_setup",
        "initialize_tournament",
        true,
        Some(sig.to_string()),
    );

    // Single-elimination registration uses the same player shard layout as Swiss.
    println!("   Step 1b: Initializing tournament player shards...");
    let mut shard_ixs = vec![ix::initialize_shards_for_size_ix(
        program_id,
        master.pubkey(),
        tournament_id,
        size,
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
        ExecutionLayer::Solana,
        "tournament_setup",
        "initialize_tournament_shards",
        true,
        Some(sig.to_string()),
    );

    // Initialize escrow before registration and fund_sol_prize.
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
        ExecutionLayer::Solana,
        "tournament_setup",
        "initialize_tournament_escrow",
        true,
        Some(sig.to_string()),
    );

    // Fund guaranteed prizes before registration.
    println!("   Step 1d: Funding guaranteed SOL prize pool...");
    let mut fund_ixs = vec![ix::fund_sol_prize_ix(
        program_id,
        master.pubkey(),
        tournament_id,
        TOURNAMENT_PRIZE_LAMPORTS,
    )?];
    apply_compute_budget(&mut fund_ixs, 150_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx =
        Transaction::new_signed_with_payer(&fund_ixs, Some(&master.pubkey()), &[master], blockhash);
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "tournament_setup",
        "fund_sol_prize",
        true,
        Some(sig.to_string()),
    );

    // Register in descending ELO order so player order matches seeding.
    println!(
        "   Step 2: Initializing profiles and registering {} players...",
        size
    );
    for (i, player) in players.iter().enumerate() {
        let username = format!("se{}{}", i, tournament_id % 1_000_000);
        let profile_ix = ix::init_profile_ix(
            program_id,
            player.pubkey(),
            username,
            "GB".to_string(),
            631_152_000, // 1990-01-01T00:00:00Z — on-chain age check requires date_of_birth > 0
        )?;
        let mut ixs = vec![profile_ix];
        apply_compute_budget(&mut ixs, 200_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx =
            Transaction::new_signed_with_payer(&ixs, Some(&player.pubkey()), &[player], blockhash);
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            ExecutionLayer::Solana,
            "profile",
            "init_profile",
            true,
            Some(sig.to_string()),
        );

        let elo = 2_800u32.saturating_sub(50 * i as u32);
        let mut reg_ixs = vec![ix::register_player_ix(
            program_id,
            player.pubkey(),
            master.pubkey(),
            tournament_id,
            size,
            elo,
        )?];
        apply_compute_budget(&mut reg_ixs, 200_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        // Only player signs; host_treasury is constrained but not a signer.
        let tx = Transaction::new_signed_with_payer(
            &reg_ixs,
            Some(&player.pubkey()),
            &[player],
            blockhash,
        );
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            ExecutionLayer::Solana,
            "tournament_setup",
            "register_player",
            true,
            Some(sig.to_string()),
        );
    }

    println!("   Step 3: Starting tournament...");
    let mut start_ixs = vec![ix::start_tournament_ix(
        program_id,
        master.pubkey(),
        master.pubkey(),
        tournament_id,
        size,
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
        ExecutionLayer::Solana,
        "tournament_setup",
        "start_tournament",
        true,
        Some(sig.to_string()),
    );

    // Round-1 match i pairs seed i with size-1-i, matching the on-chain bracket layout.
    println!("   Step 4: Initializing bracket matches...");
    let total_matches = size - 1;
    let seeded: Vec<Pubkey> = players.iter().map(|p| p.pubkey()).collect();
    let round1 = (size / 2) as usize;
    for i in 0..total_matches {
        let (round, next, slot) = ix::bracket_position(size, i);
        let (white, black) = if (i as usize) < round1 {
            (
                Some(seeded[i as usize]),
                Some(seeded[size as usize - 1 - i as usize]),
            )
        } else {
            (None, None) // filled by advance_winner as earlier rounds resolve
        };
        let mut match_ixs = vec![ix::initialize_match_ix(
            program_id,
            master.pubkey(),
            tournament_id,
            i,
            round,
            white,
            black,
            next,
            slot,
        )?];
        apply_compute_budget(&mut match_ixs, 200_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &match_ixs,
            Some(&master.pubkey()),
            &[master],
            blockhash,
        );
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            ExecutionLayer::Solana,
            "tournament_bracket",
            "initialize_match",
            true,
            Some(sig.to_string()),
        );
    }

    // The higher seed wins. The final match automatically completes the tournament.
    println!("   Step 5: Playing the bracket...");
    let mut slots: Vec<(Option<Pubkey>, Option<Pubkey>)> = (0..total_matches)
        .map(|i| {
            if (i as usize) < round1 {
                (
                    Some(seeded[i as usize]),
                    Some(seeded[size as usize - 1 - i as usize]),
                )
            } else {
                (None, None)
            }
        })
        .collect();

    for i in 0..total_matches {
        let (Some(white), Some(black)) = slots[i as usize] else {
            anyhow::bail!("match {i} has an unfilled slot — bracket advancement failed");
        };
        let (winner, loser) =
            if seeded.iter().position(|p| *p == white) < seeded.iter().position(|p| *p == black) {
                (white, black)
            } else {
                (black, white)
            };

        let mut result_ixs = vec![ix::record_match_result_ix(
            program_id,
            master.pubkey(),
            tournament_id,
            i,
            winner,
            loser,
        )?];
        apply_compute_budget(&mut result_ixs, 150_000, 10_000, 262_144);
        let blockhash = base_rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &result_ixs,
            Some(&master.pubkey()),
            &[master],
            blockhash,
        );
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            ExecutionLayer::Solana,
            "tournament_bracket",
            "record_match_result",
            true,
            Some(sig.to_string()),
        );

        let (_, next, slot) = ix::bracket_position(size, i);
        if let Some(next_idx) = next {
            let mut adv_ixs = vec![ix::advance_winner_ix(
                program_id,
                master.pubkey(),
                tournament_id,
                i,
                next_idx,
            )?];
            apply_compute_budget(&mut adv_ixs, 150_000, 10_000, 262_144);
            let blockhash = base_rpc.get_latest_blockhash()?;
            let tx = Transaction::new_signed_with_payer(
                &adv_ixs,
                Some(&master.pubkey()),
                &[master],
                blockhash,
            );
            let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
            logger.log(
                ExecutionLayer::Solana,
                "tournament_bracket",
                "advance_winner",
                true,
                Some(sig.to_string()),
            );

            if slot == 0 {
                slots[next_idx as usize].0 = Some(winner);
            } else {
                slots[next_idx as usize].1 = Some(winner);
            }
        } else {
            println!("     Champion: {}", winner);
        }
    }

    println!("   Step 6: Verifying tournament completion and distributing prizes...");
    let (status, places) =
        crate::chain::fetch_tournament_places(base_rpc, program_id, tournament_id)?;
    anyhow::ensure!(
        status == TOURNAMENT_STATUS_COMPLETED,
        "expected tournament status Completed ({TOURNAMENT_STATUS_COMPLETED}) after the final \
         match resolved, got {status}"
    );
    verify_tournament_payout(
        logger,
        base_rpc,
        program_id,
        master,
        tournament_id,
        &places,
        &prize_shares[..3],
    )
    .await?;

    // PrizesOutstanding on close indicates a failure: every funded place has been paid.
    println!("   Step 7: Closing tournament...");
    let mut close_ixs = vec![ix::close_tournament_ix(
        program_id,
        master.pubkey(),
        tournament_id,
    )?];
    apply_compute_budget(&mut close_ixs, 300_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &close_ixs,
        Some(&master.pubkey()),
        &[master],
        blockhash,
    );
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "tournament_payout",
        "close_tournament",
        true,
        Some(sig.to_string()),
    );
    println!("   Tournament closed: {}", sig);

    println!("   Single-elimination tournament flow complete!");
    Ok(())
}
