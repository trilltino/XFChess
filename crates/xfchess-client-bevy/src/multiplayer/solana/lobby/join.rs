use super::*;

pub fn spawn_lookup_game(
    rpc_url: String,
    game_id: u64,
    tx: oneshot::Sender<Result<(u64, u64), String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    crate::core::net_task::spawn_detached("lobby", move || {
        futures_lite::future::block_on(async move {
            let result = async_lookup_game(rpc_url, program_id, game_id).await;
            let _ = tx.send(result);
        })
    });
}

pub fn spawn_poll_game_start(game_id: u64, tx: oneshot::Sender<Result<(), String>>) {
    crate::core::net_task::spawn_detached("lobby", move || {
        futures_lite::future::block_on(async move {
            let result = async_poll_game_start(game_id).await;
            let _ = tx.send(result);
        })
    });
}

pub fn spawn_poll_opponent_joined(
    rpc_url: String,
    game_id: u64,
    tx: oneshot::Sender<Result<(), String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    crate::core::net_task::spawn_detached("lobby", move || {
        let _ = tx.send(poll_opponent_joined(rpc_url, program_id, game_id));
    });
}

pub fn spawn_join_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    game_id: u64,
    global_session_keypair_bytes: Option<Vec<u8>>,
    tx: oneshot::Sender<Result<u64, String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    crate::core::net_task::spawn_detached("lobby", move || {
        futures_lite::future::block_on(async move {
            let result = async_join_game(
                rpc_url,
                wallet_pubkey,
                program_id,
                game_id,
                global_session_keypair_bytes,
            )
            .await;
            if let Ok(game_id) = &result {
                crate::multiplayer::solana::wager_recovery::record(*game_id);
            }
            let _ = tx.send(result);
        })
    });
}

/// Polls until an opponent joins the game; blocks, so it runs on its own thread.
pub(super) fn poll_opponent_joined(
    rpc_url: String,
    program_id: solana_sdk::pubkey::Pubkey,
    game_id: u64,
) -> Result<(), String> {
    use std::time::{Duration, Instant};
    const POLL_INTERVAL: Duration = Duration::from_secs(3);
    const TIMEOUT: Duration = Duration::from_secs(300);
    const BLACK_OFFSET: usize = 8 + 8 + 32; // disc + game_id + white pubkey

    let rpc = solana_client::rpc_client::RpcClient::new_with_commitment(
        rpc_url,
        CommitmentConfig::confirmed(),
    );
    let game_pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let start = Instant::now();
    loop {
        if start.elapsed() > TIMEOUT {
            return Err("Timed out waiting for opponent (5 min)".to_string());
        }

        match rpc.get_account_data(&game_pda) {
            Ok(data) if data.len() >= BLACK_OFFSET + 32 => {
                let black_bytes = &data[BLACK_OFFSET..BLACK_OFFSET + 32];
                let default_bytes = [0u8; 32];
                if black_bytes != default_bytes {
                    return Ok(());
                }
            }
            Ok(_) => {}
            Err(e) => {
                return Err(format!("poll RPC: {}", e));
            }
        }

        // This runs on its own background thread, so a blocking sleep is fine.
        std::thread::sleep(POLL_INTERVAL);
    }
}

pub(super) async fn async_poll_game_start(game_id: u64) -> Result<(), String> {
    use std::time::{Duration, Instant};
    const POLL_INTERVAL: Duration = Duration::from_secs(2);
    const TIMEOUT: Duration = Duration::from_secs(600);

    let target = game_id.to_string();
    let start = Instant::now();
    loop {
        if start.elapsed() > TIMEOUT {
            return Err("Timed out waiting for the host to start (10 min)".to_string());
        }

        match crate::multiplayer::vps_client::p2p_list_games() {
            Ok(games) => {
                if games
                    .iter()
                    .any(|g| g.game_id == target && g.status == "InProgress")
                {
                    return Ok(());
                }
            }
            Err(e) => warn!("[LOBBY] poll for host start failed: {}", e),
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}

pub(super) async fn async_lookup_game(
    rpc_url: String,
    program_id: solana_sdk::pubkey::Pubkey,
    game_id: u64,
) -> Result<(u64, u64), String> {
    let game_pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let data = rpc
        .get_account_data(&game_pda)
        .map_err(|e| format!("get_account: {}", e))?;

    // Borsh Game status offset: discriminator(8) + game_id(8) + white(32) + black(32) = 80.
    const STATUS_OFFSET: usize = 8 + 8 + 32 + 32;
    if data.len() < STATUS_OFFSET + 1 {
        return Err("Account data too short for status".to_string());
    }
    let status_byte = data[STATUS_OFFSET];
    // GameStatus (Borsh discriminant order — see state/game.rs):
    // 0=Pending 1=WaitingForOpponent 2=Active 3=Inactive 4=Disputed
    // 5=Finished 6=Settled 7=Expired 8=Cancelled
    if status_byte != 1 {
        let label = match status_byte {
            0 => "pending",
            2 => "already full (Active)",
            3 => "Inactive",
            4 => "Disputed",
            5 => "Finished",
            6 => "Settled",
            7 => "Expired",
            8 => "Cancelled",
            _ => "unknown status",
        };
        return Err(format!(
            "Game {} is not available to join: {}",
            game_id, label
        ));
    }

    // Keep WAGER_OFFSET aligned with the on-chain wager_amount_offset_is_212 test.
    const WAGER_OFFSET: usize = 8 + 212;
    if data.len() < WAGER_OFFSET + 8 {
        return Err("Account data too short to read wager_amount".to_string());
    }
    let wager_lamports = u64::from_le_bytes(
        data[WAGER_OFFSET..WAGER_OFFSET + 8]
            .try_into()
            .map_err(|_| "slice error")?,
    );
    Ok((wager_lamports, game_id))
}

pub(super) async fn async_join_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: solana_sdk::pubkey::Pubkey,
    game_id: u64,
    global_session_keypair_bytes: Option<Vec<u8>>,
) -> Result<u64, String> {
    use crate::multiplayer::solana::tauri_signer::sign_via_tauri_only;
    use crate::multiplayer::vps_client;

    // Gate: joining any on-chain game requires the wager eligibility checks.
    crate::multiplayer::network::vps::identity::require_wager_eligibility(
        &wallet_pubkey.to_string(),
    )?;

    if let Some(kp_bytes) = global_session_keypair_bytes {
        return async_join_game_via_global_session(
            rpc_url,
            wallet_pubkey,
            program_id,
            game_id,
            kp_bytes,
        )
        .await;
    }

    // Get-or-create returns the session key already recorded in game.fee_payer.
    let (session_pubkey_str, _) = vps_client::create_session(game_id, &wallet_pubkey.to_string())?;
    let session_pubkey: Pubkey = session_pubkey_str
        .parse()
        .map_err(|e| format!("parse session_pubkey: {e}"))?;

    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let rpc = solana_client::rpc_client::RpcClient::new_with_commitment(
        rpc_url.clone(),
        CommitmentConfig::confirmed(),
    );
    let game_data = rpc
        .get_account_data(&game_pda)
        .map_err(|e| format!("fetch game account: {e}"))?;
    // Game layout: 8 disc + 8 game_id + 32 white pubkey
    const WHITE_OFFSET: usize = 8 + 8;
    if game_data.len() < WHITE_OFFSET + 32 {
        return Err("game account too small to read white pubkey".to_string());
    }
    let white_bytes: [u8; 32] = game_data[WHITE_OFFSET..WHITE_OFFSET + 32]
        .try_into()
        .map_err(|_| "bad white bytes".to_string())?;
    let white_player = Pubkey::from(white_bytes);

    let join_ix = join_game_ix(
        program_id,
        wallet_pubkey,
        white_player,
        session_pubkey,
        game_id,
    )
    .map_err(|e| format!("build join_game_ix: {e}"))?;
    let auth_ix =
        authorize_session_key_ix(program_id, wallet_pubkey, game_id, session_pubkey, 86400)
            .map_err(|e| format!("build authorize_session_key_ix: {e}"))?;

    let ixs = vec![join_ix, auth_ix];

    // ONE wallet popup — signs everything together.
    let signed_bytes = sign_via_tauri_only(
        &rpc_url,
        wallet_pubkey,
        &ixs,
        &[],
        &[session_pubkey],
        "Joining game",
    )
    .map_err(|e| format!("sign bundled TX: {e}"))?;

    // VPS adds its session key co-signature and submits.
    vps_client::activate_session(game_id, &signed_bytes)?;

    Ok(game_id)
}

pub(super) async fn async_join_game_via_global_session(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
    session_keypair_bytes: Vec<u8>,
) -> Result<u64, String> {
    use crate::multiplayer::solana::global_session_manager::{
        build_global_join_game_ix, check_global_session_can_afford_wager, find_global_session_pda,
    };
    use crate::solana::instructions::{PROFILE_SEED, WAGER_ESCROW_SEED};
    use solana_sdk::signature::{Keypair, Signer};

    let session_kp = Keypair::try_from(session_keypair_bytes.as_slice())
        .map_err(|e| format!("session keypair: {e}"))?;
    let (session_pda, _bump) = find_global_session_pda(&program_id, &wallet_pubkey);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, wallet_pubkey.as_ref()], &program_id).0;

    let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());

    // Fetch the white player and blockhash concurrently; neither RPC read depends on the other.
    let game_data_result = std::thread::scope(|scope| {
        let blockhash_handle =
            scope.spawn(|| rpc.get_latest_blockhash().map_err(|e| e.to_string()));
        let game_data = rpc
            .get_account_data(&game_pda)
            .map_err(|e| format!("fetch game account: {e}"));
        let blockhash = blockhash_handle
            .join()
            .map_err(|_| "blockhash fetch thread panicked".to_string())?
            .map_err(|e| format!("get_latest_blockhash: {e}"))?;
        game_data.map(|data| (data, blockhash))
    });
    let (game_data, blockhash) = game_data_result?;

    const WHITE_OFFSET: usize = 8 + 8;
    if game_data.len() < WHITE_OFFSET + 32 {
        return Err("game account too small to read white pubkey".to_string());
    }
    let white_bytes: [u8; 32] = game_data[WHITE_OFFSET..WHITE_OFFSET + 32]
        .try_into()
        .map_err(|_| "bad white bytes".to_string())?;
    let white_player = Pubkey::from(white_bytes);
    let white_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, white_player.as_ref()], &program_id).0;

    // Read the wager from the fetched account at WAGER_OFFSET (212);
    // the on-chain wager_amount_offset_is_212 test pins this layout.
    const WAGER_AMOUNT_OFFSET: usize = 8 + 212;
    let wager_lamports = if game_data.len() >= WAGER_AMOUNT_OFFSET + 8 {
        u64::from_le_bytes(
            game_data[WAGER_AMOUNT_OFFSET..WAGER_AMOUNT_OFFSET + 8]
                .try_into()
                .unwrap_or_default(),
        )
    } else {
        0
    };
    check_global_session_can_afford_wager(&rpc_url, &session_pda, wager_lamports)?;

    let ix = build_global_join_game_ix(
        &program_id,
        &session_pda,
        &session_kp.pubkey(),
        &wallet_pubkey,
        &game_pda,
        &player_profile_pda,
        &white_profile_pda,
        &escrow_pda,
        game_id,
    );

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&session_kp.pubkey()),
        &[&session_kp],
        blockhash,
    );
    crate::multiplayer::solana::submit::submit_and_poll(
        &rpc,
        &tx,
        crate::multiplayer::solana::submit::SubmitConfig::fast(),
    )
    .map_err(|e| format!("global_join_game submit: {e}"))?;

    info!(
        "[JOIN_GAME] global_join_game landed for game {} (session-signed, no wallet popup)",
        game_id
    );

    // Best-effort: lets settlement_worker discover this game — see
    // `track_global_session_game`'s doc comment.
    if let Err(e) = crate::multiplayer::vps_client::track_global_session_game(
        game_id,
        &wallet_pubkey.to_string(),
    ) {
        warn!("[JOIN_GAME] track_global_session_game failed for {game_id}: {e}");
    }

    Ok(game_id)
}
