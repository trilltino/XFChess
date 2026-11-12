use super::*;

pub(super) const MAX_JOINER_DELEGATION_WAIT_SECS: u64 = 60;

pub(super) fn handle_game_start_delegation(
    mut game_started_events: MessageReader<GameStartedEvent>,
    mut bridge: ResMut<RollupNetworkBridge>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    rollup_manager: Res<EphemeralRollupManager>,
) {
    for event in game_started_events.read() {
        // Use rollup_manager’s on-chain game ID; event.game_id identifies the gossip session.

        let game_id = if rollup_manager.game_id != 0 {
            rollup_manager.game_id
        } else {
            warn!(
                "[DELEGATION] rollup_manager.game_id is 0 at GameStarted (p2p id {}); deferring",
                event.game_id
            );
            continue;
        };

        // Only white delegates; concurrent delegation changes PDA ownership and fails the second transaction.
        if !rollup_manager.is_creator {
            // The joiner polls Game PDA ownership to observe delegation because the
            // host's local delegation task cannot update the joiner's resolver.
            if magicblock_resolver.is_delegated()
                || bridge.joiner_delegation_wait_game_id == Some(game_id)
            {
                continue;
            }

            let rpc_client = match magicblock_resolver.solana_rpc.clone() {
                Some(client) => client,
                None => {
                    error!("[DELEGATION] No Solana RPC client configured (joiner wait)");
                    continue;
                }
            };

            info!(
                "[DELEGATION] Game {} — joiner does not delegate; waiting to observe creator's delegation",
                game_id
            );

            let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
            let game_pda =
                Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;

            bridge.joiner_delegation_wait_game_id = Some(game_id);
            let (tx, rx) = oneshot::channel();
            bridge.joiner_delegation_wait_rx = Some(rx);

            crate::core::net_task::spawn_detached("bridge", move || {
                let _ = tx.send(wait_for_delegation(game_pda, game_id, rpc_client));
            });

            continue;
        }

        info!(
            "[DELEGATION] Game {} started - spawning ER delegation task",
            game_id
        );

        // Derive the game PDA using the Solana game_id
        let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
        let game_pda =
            Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;

        // Need wallet pubkey to satisfy on-chain payer == game.white || game.black check
        let wallet_pubkey = match solana_state.as_ref().and_then(|s| s.wallet_pubkey) {
            Some(pk) => pk,
            None => {
                warn!(
                    "[DELEGATION] No wallet pubkey for game {} — deferring",
                    game_id
                );
                bridge.pending_delegation_pda = Some(game_pda);
                bridge.pending_game_id = Some(game_id);
                continue;
            }
        };

        let rpc_client = match magicblock_resolver.solana_rpc.clone() {
            Some(client) => client,
            None => {
                error!("[DELEGATION] No Solana RPC client configured");
                bridge.pending_delegation_pda = Some(game_pda);
                bridge.pending_game_id = Some(game_id);
                continue;
            }
        };

        // Choose the signer from this game's used_global_session, not the wallet's
        // current session flag. Per-game sessions delegate through the VPS.
        let global_session_keypair_bytes = solana_state
            .as_ref()
            .filter(|_| rollup_manager.used_global_session)
            .and_then(|s| s.global_session_keypair.as_ref())
            .map(|kp| kp.to_bytes().to_vec());
        let _ = wallet_pubkey; // only used above to gate readiness

        // Keep the pending game PDA on all failures so delegation can emit an error
        // and retry. Clear it only after confirmed success.
        bridge.pending_delegation_pda = Some(game_pda);
        bridge.pending_game_id = Some(game_id);

        let (tx, rx) = oneshot::channel();
        bridge.delegation_rx = Some(rx);

        crate::core::net_task::spawn_detached("bridge", move || {
            futures_lite::future::block_on(async move {
                let result = spawn_delegation_task(
                    game_pda,
                    game_id,
                    rpc_client,
                    global_session_keypair_bytes,
                )
                .await;
                let _ = tx.send(result);
            })
        });

        // Item 5: fetch on-chain nonce so we never start with a stale local nonce.
        bridge.request_nonce_resync(game_id);
    }
}

pub(super) async fn spawn_delegation_task(
    game_pda: Pubkey,
    game_id: u64,
    rpc_client: Arc<RpcClient>,
    global_session_keypair_bytes: Option<Vec<u8>>,
) -> Result<Pubkey, String> {
    use solana_sdk::signer::Signer;

    info!(
        "[DELEGATION-TASK] Starting delegation for game {} (PDA: {})",
        game_id, game_pda
    );

    let mut resolver = crate::multiplayer::rollup::magicblock::MagicBlockResolver::default();
    resolver.set_solana_rpc(rpc_client.clone());
    resolver.set_game_id(game_id);

    if let Some(kp_bytes) = global_session_keypair_bytes {
        // The global session key stored in game.fee_payer satisfies both payer
        // and fee_payer locally, without a wallet round trip.
        let session_kp = solana_sdk::signature::Keypair::try_from(kp_bytes.as_slice())
            .map_err(|e| format!("session keypair: {e}"))?;
        let ix = resolver
            .create_delegation_instruction(game_pda, session_kp.pubkey(), session_kp.pubkey())
            .map_err(|e| format!("build delegation ix: {}", e))?;
        // Use fast submit-and-poll to avoid an extra preflight RPC round trip.
        use crate::multiplayer::solana::submit::{submit_local_tx, SubmitConfig};
        return match submit_local_tx(&rpc_client, &session_kp, &[ix], SubmitConfig::fast()) {
            Ok(sig) => {
                info!(
                    "[DELEGATION-TASK] SUCCESS for game {} sig: {} (session-signed, no wallet popup)",
                    game_id, sig
                );
                Ok(game_pda)
            }
            Err(e) => {
                error!("[DELEGATION-TASK] FAILED for game {}: {}", game_id, e);
                Err(e)
            }
        };
    }

    // The VPS holds the per-game fee payer key and must sign delegation for that flow.
    match crate::multiplayer::vps_client::vps_delegate_game(game_id) {
        Ok(sig) => {
            info!(
                "[DELEGATION-TASK] SUCCESS for game {} sig: {} (VPS-signed, no wallet popup)",
                game_id, sig
            );
            Ok(game_pda)
        }
        Err(e) => {
            error!("[DELEGATION-TASK] FAILED for game {}: {}", game_id, e);
            Err(e)
        }
    }
}

/// Polls until the game account is owned by the delegation program; blocks, so
/// it runs on its own thread.
pub(super) fn wait_for_delegation(
    game_pda: Pubkey,
    game_id: u64,
    rpc_client: Arc<RpcClient>,
) -> Result<Pubkey, String> {
    use crate::multiplayer::rollup::magicblock::DELEGATION_PROGRAM_ID;

    let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID
        .parse()
        .map_err(|_| "bad delegation program id".to_string())?;

    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(MAX_JOINER_DELEGATION_WAIT_SECS);
    loop {
        match rpc_client.get_account(&game_pda) {
            Ok(acc) if acc.owner == delegation_program_id => {
                info!(
                    "[DELEGATION] Game {} observed delegated (joiner) — PDA owner is now the delegation program",
                    game_id
                );
                return Ok(game_pda);
            }
            Ok(_) => {}  // not delegated yet
            Err(_) => {} // transient RPC error — keep polling
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "game {} not observed delegated after {}s",
                game_id, MAX_JOINER_DELEGATION_WAIT_SECS
            ));
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

pub(super) fn poll_joiner_delegation_wait(
    mut bridge: ResMut<RollupNetworkBridge>,
    mut magicblock_resolver: ResMut<MagicBlockResolver>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
) {
    if let Some(ref mut rx) = bridge.joiner_delegation_wait_rx {
        match rx.try_recv() {
            Ok(Ok(game_pda)) => {
                info!("Delegation observed by joiner for game {}", game_pda);
                magicblock_resolver.delegation_status = DelegationStatus::Delegated;
                magicblock_resolver.delegated_game_pda = Some(game_pda);
                magicblock_events.write(MagicBlockEvent::GameDelegated { game_pda });
                // The joiner (and a resumed client) records moves too; start
                // from the chain's nonce, not a local counter that may be 0.
                if let Some(game_id) = bridge.joiner_delegation_wait_game_id {
                    bridge.request_nonce_resync(game_id);
                }
                bridge.joiner_delegation_wait_rx = None;
                bridge.joiner_delegation_wait_game_id = None;
            }
            Ok(Err(e)) => {
                error!("[DELEGATION] Joiner delegation wait failed: {}", e);
                bridge.joiner_delegation_wait_rx = None;
                bridge.joiner_delegation_wait_game_id = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                // Still waiting, nothing to do.
            }
            Err(oneshot::error::TryRecvError::Closed) => {
                error!("[DELEGATION] Joiner delegation wait task dropped");
                bridge.joiner_delegation_wait_rx = None;
                bridge.joiner_delegation_wait_game_id = None;
            }
        }
    }
}

pub(super) fn poll_delegation_tasks(
    mut bridge: ResMut<RollupNetworkBridge>,
    mut magicblock_resolver: ResMut<MagicBlockResolver>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
) {
    if let Some(ref mut rx) = bridge.delegation_rx {
        match rx.try_recv() {
            Ok(Ok(game_pda)) => {
                info!("Delegation completed for game {}", game_pda);
                magicblock_resolver.delegation_status = DelegationStatus::Delegated;
                magicblock_resolver.delegated_game_pda = Some(game_pda);
                magicblock_events.write(MagicBlockEvent::GameDelegated { game_pda });
                bridge.delegation_rx = None;
                // Clear pending identifiers only on confirmed success so failures can be retried.
                bridge.pending_delegation_pda = None;
                bridge.pending_game_id = None;
            }
            Ok(Err(e)) => {
                error!("Delegation failed: {}", e);
                if let Some(pda) = bridge.pending_delegation_pda {
                    magicblock_events.write(MagicBlockEvent::DelegationFailed {
                        game_pda: pda,
                        error: e,
                    });
                }
                bridge.delegation_rx = None;
                bridge.delegation_retry_cooldown = 30.0;
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                // Task still running, nothing to do
            }
            Err(_) => {
                error!("Delegation task dropped");
                if let Some(pda) = bridge.pending_delegation_pda {
                    magicblock_events.write(MagicBlockEvent::DelegationFailed {
                        game_pda: pda,
                        error: "delegation task dropped before completing".to_string(),
                    });
                }
                bridge.delegation_rx = None;
                bridge.delegation_retry_cooldown = 30.0;
            }
        }
    }
}

pub(super) fn retry_pending_delegation(
    mut bridge: ResMut<RollupNetworkBridge>,
    time: Res<Time>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    rollup_manager: Res<EphemeralRollupManager>,
    magicblock_events: MessageWriter<MagicBlockEvent>,
) {
    if bridge.delegation_rx.is_some() {
        return;
    }

    // Back off after signing or broadcast failures to avoid retrying on every frame.
    if bridge.delegation_retry_cooldown > 0.0 {
        bridge.delegation_retry_cooldown -= time.delta_secs();
        return;
    }

    let game_pda = match bridge.pending_delegation_pda {
        Some(pda) => pda,
        None => return,
    };

    let game_id = match bridge.pending_game_id {
        Some(id) => id,
        None => return,
    };

    let wallet_pubkey = match solana_state.as_ref().and_then(|s| s.wallet_pubkey) {
        Some(pk) => pk,
        None => return, // wallet not ready yet; try next frame
    };

    let rpc_client = match magicblock_resolver.solana_rpc.clone() {
        Some(client) => client,
        None => {
            error!("No Solana RPC client configured for retry delegation");
            return;
        }
    };

    // Retain pending game identifiers until confirmed success so another retry can find the game.

    // Same per-game gating as `handle_game_start_delegation` — see its
    // comment for why this can't be the live `global_session_active` flag.
    let global_session_keypair_bytes = solana_state
        .as_ref()
        .filter(|_| rollup_manager.used_global_session)
        .and_then(|s| s.global_session_keypair.as_ref())
        .map(|kp| kp.to_bytes().to_vec());
    let _ = wallet_pubkey; // only used above to gate readiness

    let (tx, rx) = oneshot::channel();
    bridge.delegation_rx = Some(rx);

    crate::core::net_task::spawn_detached("bridge", move || {
        futures_lite::future::block_on(async move {
            let result =
                spawn_delegation_task(game_pda, game_id, rpc_client, global_session_keypair_bytes)
                    .await;
            let _ = tx.send(result);
        })
    });

    info!(
        "Retry delegation spawned for game {} PDA {}",
        game_id, game_pda
    );

    let _ = magicblock_events; // suppress unused warning
}

pub(super) fn handle_magic_block_events(
    mut magicblock_events: MessageReader<MagicBlockEvent>,
    mut popup_queue: ResMut<crate::ui::menus::popup::GamePopupQueue>,
) {
    for event in magicblock_events.read() {
        match event {
            MagicBlockEvent::GameDelegated { game_pda } => {
                info!("Magic Block: Game {} delegated to ER", game_pda);
            }
            MagicBlockEvent::GameUndelegated { game_pda } => {
                info!("Magic Block: Game {} undelegated from ER", game_pda);
            }
            MagicBlockEvent::DelegationFailed { game_pda, error } => {
                error!(
                    "Magic Block: Failed to delegate game {}: {}",
                    game_pda, error
                );
                // Delegation failure is retried by the client and the backend settlement worker.
                popup_queue.push(crate::ui::menus::popup::GamePopup {
                    title: "Ephemeral Rollup sync issue".to_string(),
                    message: "Having trouble syncing this game to the Ephemeral Rollup — retrying automatically.".to_string(),
                    copy_text: None,
                    url: None,
                    url_label: None,
                    lifetime: 8.0,
                    remaining: 8.0,
                    dismissed: false,
                    created_at: std::time::Instant::now(),
                });
            }
            MagicBlockEvent::UndelegationFailed { game_pda, error } => {
                error!(
                    "Magic Block: Failed to undelegate game {}: {}",
                    game_pda, error
                );
            }
            MagicBlockEvent::TransactionRoutedToEr { signature } => {
                info!("Magic Block: Transaction routed to ER: {}", signature);
            }
        }
    }
}
