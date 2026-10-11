use super::*;

pub fn sync_session_key_to_network(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut network_state: ResMut<OnlineNetworkState>,
) {
    if network_state.session_signing_key.is_some() {
        return;
    }
    if solana_state.wallet_pubkey.is_none() {
        // Casual play still requires signed gossip. Use the persistent device key
        // when no wallet is connected.
        let node_key = crate::multiplayer::network::identity::load_or_create();
        let sk: [u8; 32] = node_key.to_bytes();
        network_state.session_signing_key = Some(sk);
        if let Ok(mut shared) = network_state.session_signing_key_shared.write() {
            *shared = Some(sk);
        }
        info!(
            "[SESSION] No wallet connected — using persistent node identity as gossip-signing key"
        );
        return;
    }
    if solana_state.session_keypair.is_none() {
        // Persisted per wallet (see `network::device_id::wallet_gossip_seed`)
        // so a restarted client keeps the signer its opponent already knows.
        let wallet = solana_state
            .wallet_pubkey
            .map(|pk| pk.to_string())
            .unwrap_or_default();
        let session_kp = solana_sdk::signature::Keypair::new_from_array(
            crate::multiplayer::network::device_id::wallet_gossip_seed(&wallet),
        );
        info!(
            "[SESSION] Generated gossip-signing key: {}",
            session_kp.pubkey()
        );
        solana_state.session_keypair = Some(session_kp);
    }
    if let Some(ref kp) = solana_state.session_keypair {
        let bytes = kp.to_bytes();
        let mut sk = [0u8; 32];
        sk.copy_from_slice(&bytes[..32]);
        network_state.session_signing_key = Some(sk);
        // The outgoing-message task reads this shared cell, not the plain
        // field above — see its doc comment.
        if let Ok(mut shared) = network_state.session_signing_key_shared.write() {
            *shared = Some(sk);
        }
        info!("[SESSION] Copied session signing key to P2P network state");
    }
}

pub fn handle_pending_solana_tasks(mut solana_state: ResMut<SolanaIntegrationState>) {
    if let Some(task) = solana_state.pending_task.take() {
        if task.is_finished() {
            let result = futures_lite::future::block_on(async {
                match task.await {
                    Ok(res) => res,
                    Err(e) => Err(format!("Task panicked or cancelled: {}", e)),
                }
            });

            match result {
                Ok(game_id) => {
                    info!("Solana transaction successful for game: {}", game_id);
                    solana_state.handshake_completed = true;
                }
                Err(e) => {
                    error!("Solana transaction failed: {}", e);
                }
            }
        } else {
            solana_state.pending_task = Some(task);
        }
    }
}

pub fn authorize_session_key_on_game_start(
    mut game_start_events: MessageReader<GameStartedEvent>,
    solana_state: Res<SolanaIntegrationState>,
    game_sync: ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
    network_state: Res<OnlineNetworkState>,
    rollup_manager: Res<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    braid: Res<crate::multiplayer::network::braid_transport::BraidTransportState>,
) {
    let session_pubkey_update = game_sync.session_pubkey_update.clone();
    for _event in game_start_events.read() {
        let wallet_pubkey = match solana_state.wallet_pubkey {
            Some(pk) => pk,
            None => {
                warn!("[SESSION] Wallet not connected");
                continue;
            }
        };

        let game_id = rollup_manager.game_id;

        if game_id == 0 {
            warn!("[SESSION] No active game_id for session broadcast");
            continue;
        }

        let msg_sender = network_state.message_sender.clone();
        // Send SessionInfo through both transports so opponent identity survives
        // a missing gossip link and remains available for finalization.
        let node_b58 = network_state
            .node_id
            .as_ref()
            .map(|id| bs58::encode(id.as_bytes()).into_string());
        // Derive the gossip signer public key from the 32-byte Ed25519 seed. The
        // participant roster must match verified message signers, not session delegation keys.
        let signing_pubkey_bytes = network_state.session_signing_key;
        let braid_heads = braid.heads();

        {
            let session_pubkey_update = session_pubkey_update.clone();
            crate::core::net_task::spawn_detached("session_info", move || {
                use crate::multiplayer::vps_client;

                let Some(signing_pubkey) = signing_pubkey_bytes.map(|seed| {
                    use ed25519_dalek::SigningKey;
                    Pubkey::new_from_array(SigningKey::from_bytes(&seed).verifying_key().to_bytes())
                }) else {
                    warn!(
                        "[SESSION] No gossip-signing key yet for game {} — cannot broadcast SessionInfo (moves would be rejected as non-participant on the peer's roster check)",
                        game_id
                    );
                    return;
                };

                let mut active = false;
                for _ in 0..60 {
                    match vps_client::session_status(game_id) {
                        Ok(s) if s.active => {
                            active = true;
                            let session_pubkey: Pubkey = match s.session_pubkey.parse() {
                                Ok(pk) => pk,
                                Err(_) => break,
                            };
                            if let Ok(mut update) = session_pubkey_update.lock() {
                                *update = Some(session_pubkey);
                            }
                            info!(
                                "[SESSION] VPS session active for game {} ({})",
                                game_id, session_pubkey
                            );

                            let expires_at = chrono::Utc::now().timestamp() + 3600;
                            let msg = NetworkMessage::SessionInfo {
                                game_id,
                                player_pubkey: wallet_pubkey,
                                session_pubkey,
                                signing_pubkey,
                                expires_at,
                            };

                            // Publish SessionInfo on both transports and chain it from the shared
                            // stream head; both players write to that stream.
                            crate::multiplayer::network::braid_transport::publish_session_info(
                                crate::multiplayer::network::vps::vps_base(),
                                game_id.to_string(),
                                node_b58.clone().unwrap_or_default(),
                                String::new(),
                                wallet_pubkey.to_string(),
                                session_pubkey.to_string(),
                                signing_pubkey.to_string(),
                                expires_at,
                                braid_heads.clone(),
                            );
                            if let Some(ref tx) = msg_sender {
                                let _ = tx.send(msg);
                            }
                            break;
                        }
                        Ok(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
                        Err(e) => {
                            warn!("[SESSION] VPS status poll error: {e}");
                            std::thread::sleep(std::time::Duration::from_secs(1));
                        }
                    }
                }
                if !active {
                    error!(
                        "[SESSION] VPS session never became active for game {}",
                        game_id
                    );
                }
            });
        }
    }
}

pub fn poll_session_pubkey_update(
    mut game_sync: ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
) {
    let session_pubkey = game_sync
        .session_pubkey_update
        .lock()
        .ok()
        .and_then(|mut update| update.take());
    if let Some(session_pubkey) = session_pubkey {
        game_sync.session_pubkey = Some(session_pubkey);
    }
}

pub fn spawn_verified_participants_fetch(
    mut game_start_events: MessageReader<GameStartedEvent>,
    mut solana_state: ResMut<SolanaIntegrationState>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    causal: Res<crate::multiplayer::types::CausalChainState>,
) {
    for event in game_start_events.read() {
        let game_id = event.game_id;
        if causal.verified_wallets.contains_key(&game_id) {
            continue;
        }
        if matches!(&solana_state.pending_participants_fetch, Some((gid, _)) if *gid == game_id) {
            continue;
        }

        // fetch_verified_participants uses blocking reqwest; run it on spawn_blocking.
        let handle = tokio_runtime.0.spawn_blocking(move || {
            crate::multiplayer::vps_client::fetch_verified_participants(game_id)
        });
        solana_state.pending_participants_fetch = Some((game_id, handle));
    }
}

pub fn poll_verified_participants_fetch(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut causal: ResMut<crate::multiplayer::types::CausalChainState>,
) {
    let Some((game_id, task)) = solana_state
        .pending_participants_fetch
        .take_if(|(_, task)| task.is_finished())
    else {
        return;
    };
    let result = futures_lite::future::block_on(async {
        match task.await {
            Ok(res) => res,
            Err(e) => Err(format!("Task panicked or cancelled: {}", e)),
        }
    });
    match result {
        Ok(Some((white, black))) => {
            info!(
                "[SESSION] Verified on-chain participants for game {}: white={} black={}",
                game_id, white, black
            );
            causal.verified_wallets.insert(game_id, (white, black));
        }
        Ok(None) => {
            debug!(
                "[SESSION] No on-chain participants for game {} — casual game, roster stays trust-first",
                game_id
            );
        }
        Err(e) => {
            warn!(
                "[SESSION] Verified-participants fetch failed for game {}: {} — roster stays trust-first",
                game_id, e
            );
        }
    }
}
