use super::*;

#[derive(bevy::prelude::Resource, Debug, Clone)]
pub struct GlobalSessionActive {
    pub session_pubkey: String,
}

#[derive(bevy::prelude::Resource)]
pub struct GlobalSessionCheckPending {
    pub rx: crossbeam_channel::Receiver<Option<String>>,
}

pub fn verify_global_session_on_menu_enter(
    solana_state: Option<bevy::prelude::Res<SolanaIntegrationState>>,
    mut commands: bevy::prelude::Commands,
) {
    let wallet = match solana_state.as_ref().and_then(|s| s.wallet_pubkey) {
        Some(pk) => pk.to_string(),
        None => return,
    };

    // Remove stale resource immediately so the banner shows "checking" state.
    commands.remove_resource::<GlobalSessionActive>();

    let (tx, rx) = crossbeam_channel::bounded::<Option<String>>(1);
    commands.insert_resource(GlobalSessionCheckPending { rx });

    crate::core::net_task::spawn_detached("systems", move || {
        use crate::multiplayer::vps_client;
        match vps_client::verify_global_session(&wallet) {
            Ok(Some(session_pubkey)) => {
                info!(
                    "[GLOBAL_SESSION] VPS confirmed active session {} for {}",
                    session_pubkey, wallet
                );
                let _ = tx.send(Some(session_pubkey));
            }
            Ok(None) => {
                info!(
                    "[GLOBAL_SESSION] No active global session on VPS for {}",
                    wallet
                );
                let _ = tx.send(None);
            }
            Err(e) => {
                warn!("[GLOBAL_SESSION] Verify failed for {}: {e}", wallet);
                let _ = tx.send(None);
            }
        }
    });
}

pub fn poll_global_session_result(
    mut commands: bevy::prelude::Commands,
    pending: Option<bevy::prelude::ResMut<GlobalSessionCheckPending>>,
) {
    let Some(pending) = pending else { return };
    match pending.rx.try_recv() {
        Ok(Some(session_pubkey)) => {
            commands.insert_resource(GlobalSessionActive { session_pubkey });
            commands.remove_resource::<GlobalSessionCheckPending>();
        }
        Ok(None) => {
            commands.remove_resource::<GlobalSessionCheckPending>();
        }
        Err(crossbeam_channel::TryRecvError::Empty) => {}
        Err(_) => {
            commands.remove_resource::<GlobalSessionCheckPending>();
        }
    }
}

pub fn authorize_global_session_if_needed(
    mut solana_state: ResMut<SolanaIntegrationState>,
    time: Res<Time>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Result<Keypair, String>>>>,
    mut retry_timer: Local<f32>,
    mut attempted_for: Local<Option<Pubkey>>,
    mut failed_attempts: Local<u32>,
) {
    // Enable global sessions only for embedded wallets; extension wallets still
    // fall back to per-game signing because cluster selection can mismatch.
    if !solana_state.wallet_is_embedded {
        return;
    }

    const MAX_ATTEMPTS: u32 = 3;

    let Some(wallet_pubkey) = solana_state.wallet_pubkey else {
        return;
    };

    if let Some(ref receiver) = *rx {
        match receiver.try_recv() {
            Ok(Ok(kp)) => {
                info!("[GLOBAL_SESSION] Authorized — session {}", kp.pubkey());
                solana_state.global_session_keypair = Some(kp);
                solana_state.global_session_active = true;
                solana_state.global_session_unavailable_reason = None;
                solana_state.global_session_setup_in_progress = false;
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new(
                        "solana_global_session_active",
                    )
                    .wallet(wallet_pubkey)
                    .session_kind("global"),
                );
                *rx = None;
            }
            Ok(Err(e)) => {
                *failed_attempts += 1;
                *rx = None;
                solana_state.global_session_setup_in_progress = false;
                if *failed_attempts >= MAX_ATTEMPTS {
                    warn!(
                        "[GLOBAL_SESSION] Authorization failed {} times ({e}) — giving up for this session, falling back to per-game wallet signing.",
                        *failed_attempts
                    );
                    *retry_timer = f32::INFINITY; // never retry again this run
                    solana_state.global_session_unavailable_reason = Some(e);
                    crate::multiplayer::network::vps::emit_client_event(
                        crate::multiplayer::network::vps::ClientEvent::new(
                            "solana_global_session_unavailable",
                        )
                        .wallet(wallet_pubkey)
                        .session_kind("per_game")
                        .reason(
                            solana_state
                                .global_session_unavailable_reason
                                .clone()
                                .unwrap_or_else(|| "authorization failed".to_string()),
                        ),
                    );
                } else {
                    warn!(
                        "[GLOBAL_SESSION] Authorization failed ({}/{MAX_ATTEMPTS}): {e}",
                        *failed_attempts
                    );
                    *retry_timer = 30.0; // back off before trying again
                }
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                *failed_attempts += 1;
                *rx = None;
                solana_state.global_session_setup_in_progress = false;
                *retry_timer = if *failed_attempts >= MAX_ATTEMPTS {
                    f32::INFINITY
                } else {
                    30.0
                };
                if *failed_attempts >= MAX_ATTEMPTS {
                    solana_state.global_session_unavailable_reason =
                        Some("background task dropped".to_string());
                    crate::multiplayer::network::vps::emit_client_event(
                        crate::multiplayer::network::vps::ClientEvent::new(
                            "solana_global_session_unavailable",
                        )
                        .wallet(wallet_pubkey)
                        .session_kind("per_game")
                        .reason("background task dropped"),
                    );
                }
            }
        }
        return;
    }

    if solana_state.global_session_active {
        return;
    }
    // Don't compete with the (separate, existing) profile-creation gate —
    // wait until that's fully done first.
    if solana_state.profile_status != super::super::state::ProfileStatus::HasProfileWithUsername {
        return;
    }
    // One attempt per connected wallet per app run, then a cooldown on failure.
    if *attempted_for == Some(wallet_pubkey) {
        *retry_timer -= time.delta_secs();
        if *retry_timer > 0.0 {
            return;
        }
    }

    // Check the wallet covers the session deposit, key funding, and rent/fees
    // before opening an authorization prompt.
    const MIN_BALANCE_FOR_GLOBAL_SESSION_SOL: f64 = 0.115;
    if solana_state.balance < MIN_BALANCE_FOR_GLOBAL_SESSION_SOL {
        solana_state.global_session_unavailable_reason = Some(format!(
            "wallet balance {:.4} SOL is below the {:.3} SOL needed to set up one-time session signing — fund the wallet and it will retry automatically",
            solana_state.balance, MIN_BALANCE_FOR_GLOBAL_SESSION_SOL
        ));
        crate::multiplayer::network::vps::emit_client_event(
            crate::multiplayer::network::vps::ClientEvent::new("solana_global_session_unavailable")
                .wallet(wallet_pubkey)
                .session_kind("per_game")
                .reason("insufficient wallet balance for global session"),
        );
        *attempted_for = Some(wallet_pubkey);
        *retry_timer = 30.0; // re-check periodically in case the wallet gets funded
        return;
    }

    *attempted_for = Some(wallet_pubkey);

    let program_id: Pubkey = crate::solana::instructions::PROGRAM_ID
        .parse()
        .unwrap_or_default();
    let rpc_url = DEVNET_RPC_URL.to_string();
    let (tx, receiver) = crossbeam_channel::bounded(1);
    *rx = Some(receiver);
    solana_state.global_session_setup_in_progress = true;
    crate::core::net_task::spawn_detached("systems", move || {
        let _ = tx.send(establish_global_session(
            wallet_pubkey,
            program_id,
            &rpc_url,
        ));
    });
}

pub(super) fn establish_global_session(
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    rpc_url: &str,
) -> Result<Keypair, String> {
    use crate::multiplayer::solana::global_session_manager::{
        build_authorize_global_session_ix, build_revoke_global_session_ix,
        build_withdraw_global_session_ix, find_global_session_pda, global_session_account_exists,
        global_session_is_live_onchain, AuthorizeGlobalSessionArgs, GlobalSessionKeyManager,
    };
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;

    let mgr = GlobalSessionKeyManager::new(&wallet_pubkey);
    let session_pubkey = mgr.pubkey();
    let (session_pda, _bump) = find_global_session_pda(&program_id, &wallet_pubkey);

    // 0.1 SOL — matches the (currently unused) backend `/global-session/prepare`
    // default; no spending_limit/max_wager cap beyond the deposit itself.
    const DEPOSIT_LAMPORTS: u64 = 100_000_000;
    let authorize_ix = build_authorize_global_session_ix(
        &program_id,
        &wallet_pubkey,
        &session_pda,
        AuthorizeGlobalSessionArgs {
            session_key: session_pubkey,
            duration_secs: None,
            spending_limit: None,
            max_wager: None,
            games: None,
            deposit_lamports: DEPOSIT_LAMPORTS,
        },
    );

    // Fund the fresh client-side session key in the same authorization transaction
    // so it can pay subsequent fees.
    const SESSION_KEY_FUND_LAMPORTS: u64 = 10_000_000;
    let fund_ix = solana_system_interface::instruction::transfer(
        &wallet_pubkey,
        &session_pubkey,
        SESSION_KEY_FUND_LAMPORTS,
    );

    // Revoke a live on-chain delegation with no matching local key before reauthorizing.
    // Retain reactive retry in case this preflight read is stale.
    let needs_revoke_first = global_session_is_live_onchain(rpc_url, &session_pda);
    if needs_revoke_first {
        info!(
            "[GLOBAL_SESSION] On-chain delegation already live (pre-flight check) — revoking before authorizing, no wasted popup"
        );
    }

    // Reclaim expired or exhausted session balances before fresh authorization
    // to avoid accumulating stranded deposits.
    let stale_balance_to_reclaim =
        !needs_revoke_first && global_session_account_exists(rpc_url, &session_pda);
    if stale_balance_to_reclaim {
        info!(
            "[GLOBAL_SESSION] Prior session account exists but is expired/exhausted — reclaiming its balance in the same re-authorize transaction"
        );
    }

    let authorize_result = if needs_revoke_first {
        Err("GlobalSessionAlreadyActive (pre-flight check)".to_string())
    } else if stale_balance_to_reclaim {
        let withdraw_ix =
            build_withdraw_global_session_ix(&program_id, &wallet_pubkey, &session_pda);
        sign_and_send_via_tauri(
            rpc_url,
            wallet_pubkey,
            &[withdraw_ix, authorize_ix.clone(), fund_ix.clone()],
            &[],
            "Setting up one-time quick-sign session",
        )
        .map(|_| ())
    } else {
        sign_and_send_via_tauri(
            rpc_url,
            wallet_pubkey,
            &[authorize_ix.clone(), fund_ix.clone()],
            &[],
            "Setting up one-time quick-sign session",
        )
        .map(|_| ())
    };

    if let Err(e) = authorize_result {
        if e.contains("GlobalSessionAlreadyActive") {
            if !needs_revoke_first {
                info!(
                    "[GLOBAL_SESSION] On-chain delegation already active with no matching local key — revoking before re-authorizing"
                );
            }
            let revoke_ix =
                build_revoke_global_session_ix(&program_id, &wallet_pubkey, &session_pda);
            // Bundle withdrawal with revoke: revoke disables the session without returning lamports.
            let withdraw_ix =
                build_withdraw_global_session_ix(&program_id, &wallet_pubkey, &session_pda);
            sign_and_send_via_tauri(
                rpc_url,
                wallet_pubkey,
                &[revoke_ix, withdraw_ix],
                &[],
                "Revoking stale quick-sign session",
            )
            .map_err(|e2| {
                format!("authorize_global_session: {e} (revoke retry also failed: {e2})")
            })?;
            sign_and_send_via_tauri(
                rpc_url,
                wallet_pubkey,
                &[authorize_ix, fund_ix],
                &[],
                "Setting up one-time quick-sign session",
            )
            .map_err(|e2| format!("authorize_global_session (after revoke): {e2}"))?;
        } else {
            return Err(format!("authorize_global_session: {e}"));
        }
    }

    register_global_session_with_backend(wallet_pubkey, mgr.signer().to_bytes());

    mgr.save(&wallet_pubkey, 30)
        .map_err(|e| format!("save session key: {e}"))?;

    Keypair::try_from(mgr.signer().to_bytes().as_slice())
        .map_err(|e| format!("keypair conversion: {e}"))
}

pub(crate) enum GlobalSessionRegisterOutcome {
    Confirmed,
    ConfirmedMismatch,
    Transient,
}

#[derive(bevy::prelude::Resource)]
pub struct GlobalSessionRegisterPending {
    pub(super) rx: crossbeam_channel::Receiver<GlobalSessionRegisterOutcome>,
}

pub fn poll_global_session_register_result(
    mut solana_state: ResMut<SolanaIntegrationState>,
    pending: Option<Res<GlobalSessionRegisterPending>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };
    match pending.rx.try_recv() {
        Ok(GlobalSessionRegisterOutcome::ConfirmedMismatch) => {
            warn!(
                "[GLOBAL_SESSION] Backend confirmed the local session key doesn't match on-chain — clearing it"
            );
            if let Some(wallet) = solana_state.wallet_pubkey {
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new(
                        "solana_global_session_register_failed",
                    )
                    .wallet(wallet)
                    .session_kind("global")
                    .reason("backend confirmed key mismatch"),
                );
            }
            solana_state.global_session_active = false;
            solana_state.global_session_keypair = None;
            commands.remove_resource::<GlobalSessionRegisterPending>();
        }
        Ok(GlobalSessionRegisterOutcome::Confirmed | GlobalSessionRegisterOutcome::Transient) => {
            commands.remove_resource::<GlobalSessionRegisterPending>();
        }
        Err(crossbeam_channel::TryRecvError::Empty) => {}
        Err(_) => {
            commands.remove_resource::<GlobalSessionRegisterPending>();
        }
    }
}

pub(super) fn register_global_session_with_backend(
    wallet_pubkey: Pubkey,
    keypair_bytes: [u8; 64],
) -> GlobalSessionRegisterOutcome {
    let secret_b58 = bs58::encode(keypair_bytes).into_string();
    let url = format!(
        "{}/api/global-session/register",
        crate::multiplayer::network::vps::vps_base()
    );
    let body = serde_json::json!({
        "wallet_pubkey": wallet_pubkey.to_string(),
        "session_secret_key_b58": secret_b58,
    });
    match reqwest::blocking::Client::new()
        .post(&url)
        .json(&body)
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            info!("[GLOBAL_SESSION] Registered session key with backend");
            GlobalSessionRegisterOutcome::Confirmed
        }
        Ok(resp)
            if resp.status() == reqwest::StatusCode::FORBIDDEN
                || resp.status() == reqwest::StatusCode::BAD_GATEWAY =>
        {
            warn!(
                "[GLOBAL_SESSION] Backend rejected registration ({}) — key does not match on-chain state",
                resp.status()
            );
            GlobalSessionRegisterOutcome::ConfirmedMismatch
        }
        Ok(resp) => {
            warn!(
                "[GLOBAL_SESSION] Backend registration returned {} — finalize/undelegate/settlement won't work for this wallet until it's retried",
                resp.status()
            );
            GlobalSessionRegisterOutcome::Transient
        }
        Err(e) => {
            warn!("[GLOBAL_SESSION] Backend registration failed: {e}");
            GlobalSessionRegisterOutcome::Transient
        }
    }
}
