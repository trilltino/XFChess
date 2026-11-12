use super::*;

pub fn spawn_create_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    wager_lamports: u64,
    match_type: u8,
    time_base: u32,
    time_inc: u32,
    global_session_keypair_bytes: Option<Vec<u8>>,
    tx: oneshot::Sender<Result<u64, String>>,
) {
    let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();

    crate::core::net_task::spawn_detached("lobby", move || {
        futures_lite::future::block_on(async move {
            let result = async_create_game(
                rpc_url,
                wallet_pubkey,
                program_id,
                wager_lamports,
                match_type,
                time_base,
                time_inc,
                global_session_keypair_bytes,
            )
            .await;
            if let Ok(game_id) = &result {
                if wager_lamports > 0 {
                    crate::multiplayer::solana::wager_recovery::record(*game_id);
                }
            }
            let _ = tx.send(result);
        })
    });
}

pub(super) async fn async_create_game(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: solana_sdk::pubkey::Pubkey,
    wager_lamports: u64,
    match_type: u8,
    time_base: u32,
    time_inc: u32,
    global_session_keypair_bytes: Option<Vec<u8>>,
) -> Result<u64, String> {
    use crate::multiplayer::solana::tauri_signer::sign_via_tauri_only;
    use crate::multiplayer::vps_client;
    use std::time::Instant;

    // Gate: only wallets with profile + email + KYC may create a wagered match.
    if wager_lamports > 0 {
        crate::multiplayer::network::vps::identity::require_wager_eligibility(
            &wallet_pubkey.to_string(),
        )?;
    }

    let game_id: u64 = rand::random();

    if let Some(kp_bytes) = global_session_keypair_bytes {
        return async_create_game_via_global_session(
            rpc_url,
            wallet_pubkey,
            program_id,
            game_id,
            wager_lamports,
            match_type,
            time_base,
            time_inc,
            kp_bytes,
        )
        .await;
    }

    let legacy_start = Instant::now();
    info!(
        "[CREATE_GAME] legacy per-game signing path selected for game {game_id} (wager_lamports={wager_lamports}, match_type={match_type})"
    );

    let step_start = Instant::now();
    let (session_pubkey_str, platform_fee_lamports) =
        vps_client::create_session(game_id, &wallet_pubkey.to_string())?;
    info!(
        "[CREATE_GAME] /session/create completed for game {game_id} in {:?}",
        step_start.elapsed()
    );
    let session_pubkey: Pubkey = session_pubkey_str
        .parse()
        .map_err(|e| format!("parse session_pubkey: {e}"))?;

    let create_ix = create_game_ix(
        program_id,
        wallet_pubkey,
        session_pubkey,
        game_id,
        wager_lamports,
        match_type,
        platform_fee_lamports,
        time_base as u64,
        time_inc as u16,
    )
    .map_err(|e| format!("build create_game_ix: {e}"))?;
    let auth_ix =
        authorize_session_key_ix(program_id, wallet_pubkey, game_id, session_pubkey, 86400)
            .map_err(|e| format!("build authorize_session_key_ix: {e}"))?;

    let ixs = vec![create_ix, auth_ix];

    let label = if wager_lamports > 0 {
        "Creating wagered game"
    } else {
        "Creating game"
    };
    let step_start = Instant::now();
    let signed_bytes =
        sign_via_tauri_only(&rpc_url, wallet_pubkey, &ixs, &[], &[session_pubkey], label)
            .map_err(|e| format!("sign bundled TX: {e}"))?;
    info!(
        "[CREATE_GAME] wallet signing completed for game {game_id} in {:?}",
        step_start.elapsed()
    );

    let step_start = Instant::now();
    vps_client::activate_session(game_id, &signed_bytes)?;
    info!(
        "[CREATE_GAME] /session/activate completed for game {game_id} in {:?}",
        step_start.elapsed()
    );

    // Session activation already waits for server-side confirmation.
    info!(
        "[CREATE_GAME] legacy per-game create finished for game {game_id} in {:?}",
        legacy_start.elapsed()
    );

    Ok(game_id)
}

pub(super) async fn async_create_game_via_global_session(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
    wager_lamports: u64,
    match_type: u8,
    time_base: u32,
    time_inc: u32,
    session_keypair_bytes: Vec<u8>,
) -> Result<u64, String> {
    use crate::multiplayer::solana::global_session_manager::{
        build_global_create_game_ix, check_global_session_can_afford_wager, find_global_session_pda,
    };
    use crate::solana::instructions::WAGER_ESCROW_SEED;
    use solana_sdk::signature::{Keypair, Signer};
    use std::time::Instant;

    let session_kp = Keypair::try_from(session_keypair_bytes.as_slice())
        .map_err(|e| format!("session keypair: {e}"))?;
    let (session_pda, _bump) = find_global_session_pda(&program_id, &wallet_pubkey);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    // Spending caps do not establish vault balance; check the wager is affordable
    // before paying a submission fee.
    check_global_session_can_afford_wager(&rpc_url, &session_pda, wager_lamports)?;

    // Free matches never pay platform fees at settlement, so skip the rate fetch.
    let platform_fee_lamports = if match_type != 0 {
        crate::multiplayer::vps_client::fetch_platform_fee_lamports()
            .map_err(|e| format!("fetch platform fee: {e}"))?
    } else {
        0
    };

    let ix = build_global_create_game_ix(
        &program_id,
        &session_pda,
        &session_kp.pubkey(),
        &wallet_pubkey,
        &game_pda,
        &escrow_pda,
        game_id,
        wager_lamports,
        match_type,
        platform_fee_lamports,
        time_base as u64,
        time_inc as u16,
    );

    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    let start = Instant::now();
    crate::multiplayer::solana::submit::submit_local_tx(
        &rpc,
        &session_kp,
        &[ix],
        crate::multiplayer::solana::submit::SubmitConfig::fast(),
    )
    .map_err(|e| format!("global_create_game submit: {e}"))?;

    info!(
        "[CREATE_GAME] global_create_game landed for game {} in {:?} (session-signed, no wallet popup)",
        game_id,
        start.elapsed()
    );

    // Track the game for automatic settlement without blocking successful creation on this request.
    if let Err(e) = crate::multiplayer::vps_client::track_global_session_game(
        game_id,
        &wallet_pubkey.to_string(),
    ) {
        warn!("[CREATE_GAME] track_global_session_game failed for {game_id}: {e}");
    }

    Ok(game_id)
}
