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
use swiss_pairing::{
    calculate_standings, generate_pairings, Color, MatchResult, PairingConfig, SwissPlayer,
    SwissRound,
};

pub async fn run_swiss_tournament_flow(
    base_rpc: &RpcClient,
    _er_rpc: &RpcClient,
    program_id: Pubkey,
    master: &Keypair,
    players: &[Keypair],
    size: u16,
    logger: &mut MeasurementLog,
) -> anyhow::Result<()> {
    let tournament_id = unique_id()?;
    println!(
        "\n   Setting up Swiss tournament #{} ({} players)",
        tournament_id, size
    );

    println!("   Step 1: Initializing tournament...");
    let rounds = (size as f64).log2().ceil() as u8 + 1;
    let mut init_ixs = vec![ix::initialize_tournament_ix(
        program_id,
        master.pubkey(),
        tournament_id,
        &format!("ER_Benchmark_{}", tournament_id),
        1_000_000,
        size,
        Some(rounds),
        0,                                          // elo_min
        3_000,                                      // elo_max
        2,                                          // min_players
        [5000u16, 3000, 2000, 0, 0, 0, 0, 0, 0, 0], // prize_shares: 50/30/20 split
        0,                                          // platform_fee
        false,                                      // winner_takes_all
        master.pubkey(),                            // host_treasury
        60, // 1 min base clock — synthetic instant-move benchmark, no real clock needed
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

    // Initialize shards in a separate transaction to avoid BPF stack overflow.
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

    // Initialize escrow before registration.
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

    // Fund the guaranteed prize before registration; the instruction rejects
    // existing prize pools and tournaments with registered players.
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
        let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
        logger.log(
            ExecutionLayer::Solana,
            "profile",
            "init_profile",
            true,
            Some(sig.to_string()),
        );
        if (i + 1) % 8 == 0 || i + 1 == size as usize {
            println!("     Profiles {}/{}", i + 1, size);
        }
    }

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

    println!("   Step 3: Starting tournament...");
    let mut start_ixs = vec![ix::start_tournament_ix(
        program_id,
        master.pubkey(),
        master.pubkey(),
        tournament_id,
        size,
    )?];
    apply_compute_budget(&mut start_ixs, 200_000, 10_000, 262_144);
    send_as_master(
        base_rpc,
        master,
        start_ixs,
        logger,
        "tournament_setup",
        "start_tournament",
    )
    .await?;

    // Pair with the production engine and send the same authority-signed
    // instructions the backend's Swiss orchestrator and game provisioner send.
    println!(
        "   Step 4: Playing {} Swiss rounds (pairing via swiss-pairing)...",
        rounds
    );
    let mut field: Vec<SwissPlayer> = players
        .iter()
        .map(|p| SwissPlayer::new(p.pubkey().to_string(), 1200))
        .collect();
    let mut played: Vec<SwissRound> = Vec::new();
    let mut results: Vec<(u8, u16, MatchResult)> = Vec::new();
    let pk = |id: &str| -> anyhow::Result<Pubkey> { Ok(id.parse()?) };

    for round in 1..=rounds {
        let paired = generate_pairings(round, &field, rounds, &PairingConfig::default())
            .map_err(|e| anyhow::anyhow!("pairing round {round}: {e}"))?;
        println!(
            "     Round {}/{}: {} boards, {} bye(s)",
            round,
            rounds,
            paired.pairings.len(),
            paired.byes.len()
        );

        for p in &paired.pairings {
            let (white, black) = (pk(&p.white)?, pk(&p.black)?);
            let game_id = (tournament_id << 16) | (u64::from(round) << 8) | u64::from(p.board);
            let session_key = Keypair::new().pubkey();

            let mut create_ixs = vec![ix::create_tournament_game_ix(
                program_id,
                master.pubkey(),
                tournament_id,
                size,
                game_id,
                white,
                black,
                session_key,
            )?];
            apply_compute_budget(&mut create_ixs, 250_000, 10_000, 262_144);
            send_as_master(
                base_rpc,
                master,
                create_ixs,
                logger,
                "tournament_game",
                "create_tournament_game",
            )
            .await?;
            verify_game_seated(base_rpc, program_id, game_id, white, black)?;

            // Synthetic result: white wins every board.
            let mut result_ixs = vec![ix::record_swiss_result_ix(
                program_id,
                master.pubkey(),
                tournament_id,
                size,
                round - 1,
                p.board - 1,
                ix::SWISS_WHITE_WIN,
                white,
                black,
            )?];
            apply_compute_budget(&mut result_ixs, 150_000, 10_000, 262_144);
            send_as_master(
                base_rpc,
                master,
                result_ixs,
                logger,
                "tournament_game",
                "record_swiss_result",
            )
            .await?;
            results.push((round, p.board, MatchResult::WhiteWin));
        }

        // Byes are board slots after the real boards.
        for (i, bye) in paired.byes.iter().enumerate() {
            let player = pk(bye)?;
            let mut bye_ixs = vec![ix::record_swiss_result_ix(
                program_id,
                master.pubkey(),
                tournament_id,
                size,
                round - 1,
                (paired.pairings.len() + i) as u16,
                ix::SWISS_BYE,
                player,
                player,
            )?];
            apply_compute_budget(&mut bye_ixs, 150_000, 10_000, 262_144);
            send_as_master(
                base_rpc,
                master,
                bye_ixs,
                logger,
                "tournament_game",
                "record_swiss_bye",
            )
            .await?;
        }

        let slots = (paired.pairings.len() + paired.byes.len()) as u16;
        let mut advance_ixs = vec![ix::advance_round_ix(
            program_id,
            master.pubkey(),
            tournament_id,
            slots,
        )?];
        apply_compute_budget(&mut advance_ixs, 150_000, 10_000, 262_144);
        send_as_master(
            base_rpc,
            master,
            advance_ixs,
            logger,
            "tournament_game",
            "advance_round",
        )
        .await?;

        apply_round(&mut field, &paired);
        played.push(paired);
    }

    let standings = calculate_standings(&field, &played, &results);
    let places = standings
        .iter()
        .take(10)
        .map(|s| pk(&s.player_id))
        .collect::<anyhow::Result<Vec<Pubkey>>>()?;

    println!("   Step 5: Completing Swiss tournament with the engine's final order...");
    let mut complete_ixs = vec![ix::complete_swiss_tournament_ix(
        program_id,
        master.pubkey(),
        tournament_id,
        size,
        &places,
    )?];
    apply_compute_budget(&mut complete_ixs, 200_000, 10_000, 262_144);
    send_as_master(
        base_rpc,
        master,
        complete_ixs,
        logger,
        "tournament_payout",
        "complete_swiss_tournament",
    )
    .await?;

    println!("   Step 6: Distributing and verifying tournament prizes...");
    let (status, places) =
        crate::chain::fetch_tournament_places(base_rpc, program_id, tournament_id)?;
    anyhow::ensure!(
        status == TOURNAMENT_STATUS_COMPLETED,
        "expected tournament status Completed ({TOURNAMENT_STATUS_COMPLETED}) after \
         complete_swiss_tournament, got {status}"
    );
    verify_tournament_payout(
        logger,
        base_rpc,
        program_id,
        master,
        tournament_id,
        &places,
        &[5000, 3000, 2000],
    )
    .await?;

    // After paying funded places, sweep rounding, unallocated shares, and rent to treasury.
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
    // All funded places have been paid; propagate close failure as an error.
    let sig = with_retry(|| base_rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        "tournament_payout",
        "close_tournament",
        true,
        Some(sig.to_string()),
    );
    println!("   Tournament closed: {}", sig);

    println!("   Swiss tournament flow complete!");
    Ok(())
}

async fn send_as_master(
    rpc: &RpcClient,
    master: &Keypair,
    ixs: Vec<solana_sdk::instruction::Instruction>,
    logger: &mut MeasurementLog,
    category: &str,
    name: &str,
) -> anyhow::Result<()> {
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&ixs, Some(&master.pubkey()), &[master], blockhash);
    let sig = with_retry(|| rpc.send_and_confirm_transaction(&tx)).await?;
    logger.log(
        ExecutionLayer::Solana,
        category,
        name,
        true,
        Some(sig.to_string()),
    );
    Ok(())
}

/// The Game exists, is program-owned, seats the pairing and is Active — i.e. a
/// client could delegate it and start playing.
fn verify_game_seated(
    rpc: &RpcClient,
    program_id: Pubkey,
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
) -> anyhow::Result<()> {
    const GAME_STATUS_ACTIVE: u8 = 2;
    let game_pda = Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;
    let account = rpc.get_account(&game_pda)?;
    anyhow::ensure!(
        account.owner == program_id,
        "game {game_id} has the wrong owner"
    );
    let d = &account.data;
    anyhow::ensure!(d.len() > 80, "game {game_id} account too short");
    anyhow::ensure!(
        d[16..48] == white.to_bytes() && d[48..80] == black.to_bytes(),
        "game {game_id} seats the wrong players"
    );
    anyhow::ensure!(
        d[80] == GAME_STATUS_ACTIVE,
        "game {game_id} status {} is not Active",
        d[80]
    );
    Ok(())
}

/// Feeds one round's outcome back into the field the way the backend rebuilds
/// it from stored results: every board is a white win, byes are a full point.
fn apply_round(field: &mut [SwissPlayer], round: &SwissRound) {
    for player in field.iter_mut() {
        if round.byes.contains(&player.id) {
            player.score += 1.0;
            player.bye_rounds.push(round.round);
        }
        for p in &round.pairings {
            if p.white == player.id {
                player.score += 1.0;
                player.opponents.push(p.black.clone());
                player.color_history.push(Color::White);
            } else if p.black == player.id {
                player.opponents.push(p.white.clone());
                player.color_history.push(Color::Black);
            }
        }
        let float = if round.float_downs.contains(&player.id) {
            swiss_pairing::FloatStatus::Down
        } else if round.float_ups.contains(&player.id) {
            swiss_pairing::FloatStatus::Up
        } else {
            swiss_pairing::FloatStatus::None
        };
        player.float_history.push(float);
    }
}
