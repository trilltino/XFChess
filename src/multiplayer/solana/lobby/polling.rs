use super::*;

pub(super) fn poll_lobby_tasks(
    mut lobby: ResMut<SolanaLobbyState>,
    mut sync: ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
    mut competitive: ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    mut p2p_vps: ResMut<crate::multiplayer::network::p2p_vps::P2PVpsState>,
) {
    // Cancellation owns both the on-chain refund and relay removal. Do not
    // transition back to Idle until the result is known to the user.
    if let Some(ref mut rx) = lobby.cancel_rx {
        match rx.try_recv() {
            Ok(Ok(outcome)) => {
                let game_id = match lobby.status {
                    LobbyStatus::Cancelling { game_id } => game_id,
                    _ => 0,
                };
                let (refunded, message) = match outcome {
                    CancelOutcome::Refunded(sig) => {
                        let sig_text = sig.to_string();
                        crate::multiplayer::solana::wager_recovery::forget(game_id);
                        lobby.money_flow =
                            crate::multiplayer::solana::money_flow::MoneyFlowState::confirmed(
                                "cancel_game",
                                Some(sig_text.clone()),
                            );
                        crate::multiplayer::network::vps::emit_client_event(
                            crate::multiplayer::network::vps::ClientEvent::new("tx_confirmed")
                                .game_id(game_id)
                                .action("cancel_game")
                                .signature(sig_text.clone())
                                .status("confirmed"),
                        );
                        if let Err(e) = crate::multiplayer::network::vps::register_money_action(
                            crate::multiplayer::network::vps::RegisterMoneyActionReq {
                                action_type: "cancel_game".to_string(),
                                scope_type: "game".to_string(),
                                game_id: Some(game_id as i64),
                                tournament_id: None,
                                wallet: None,
                                signature: Some(sig_text.clone()),
                                reason: Some("wager host/lobby cancellation".to_string()),
                            },
                        ) {
                            warn!("[money-actions] cancel_game register failed: {e}");
                        }
                        (true, format!("Wager refunded. Transaction: {sig_text}"))
                    }
                    CancelOutcome::NothingToRefund(message) => {
                        crate::multiplayer::solana::wager_recovery::forget(game_id);
                        lobby.money_flow =
                            crate::multiplayer::solana::money_flow::MoneyFlowState::confirmed(
                                "cancel_game",
                                Option::<solana_sdk::signature::Signature>::None,
                            );
                        crate::multiplayer::network::vps::emit_client_event(
                            crate::multiplayer::network::vps::ClientEvent::new(
                                "reconcile_resolved",
                            )
                            .game_id(game_id)
                            .action("cancel_game")
                            .status("confirmed")
                            .reason(message.clone()),
                        );
                        (false, message)
                    }
                };
                lobby.status = LobbyStatus::Cancelled {
                    game_id,
                    refunded,
                    message,
                };
                lobby.cancel_rx = None;
                lobby.rescan_recovery();
                lobby.opponent_poll_rx = None;
                lobby.game_start_poll_rx = None;
                let game_id = game_id.to_string();
                if p2p_vps.hosting_game_id.as_deref() == Some(game_id.as_str()) {
                    p2p_vps.hosting_game_id = None;
                }
                if p2p_vps.joining_game_id.as_deref() == Some(game_id.as_str()) {
                    p2p_vps.joining_game_id = None;
                }
                sync.game_id = None;
                competitive.game_id = None;
                competitive.active = false;
                rollup_manager.game_id = 0;
            }
            Ok(Err(error)) => {
                let game_id = match lobby.status {
                    LobbyStatus::Cancelling { game_id } => game_id,
                    _ => 0,
                };
                lobby.money_flow = crate::multiplayer::solana::money_flow::MoneyFlowState::failed(
                    "cancel_game",
                    error.clone(),
                );
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new("tx_failed")
                        .game_id(game_id)
                        .action("cancel_game")
                        .status("failed")
                        .reason(error.clone()),
                );
                lobby.status = LobbyStatus::CancelFailed { game_id, error };
                lobby.cancel_rx = None;
                lobby.rescan_recovery();
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                let game_id = match lobby.status {
                    LobbyStatus::Cancelling { game_id } => game_id,
                    _ => 0,
                };
                lobby.money_flow =
                    crate::multiplayer::solana::money_flow::MoneyFlowState::admin_review(
                        "cancel_game",
                        Option::<String>::None,
                        "cancellation task stopped unexpectedly",
                    );
                lobby.status = LobbyStatus::CancelFailed {
                    game_id,
                    error: "Cancellation task stopped unexpectedly".to_string(),
                };
                lobby.cancel_rx = None;
            }
        }
    }

    // Poll transaction receiver.
    if let Some(ref mut rx) = lobby.tx_rx {
        match rx.try_recv() {
            Ok(Ok(game_id)) => {
                sync.game_id = Some(game_id);
                sync.wager_amount = lobby.wager_lamports();
                // Zero-stake games need no ER delegation and must accept moves immediately.
                sync.requires_delegation = lobby.wager_lamports() > 0;
                competitive.wager_lamports = lobby.wager_lamports();
                competitive.stake_amount = lobby.wager_lamports();
                competitive.game_id = Some(game_id);
                competitive.active = true;
                lobby.status = LobbyStatus::Success(game_id);
                lobby.tx_rx = None;
                crate::multiplayer::network::game_id_store::set(game_id);
                rollup_manager.assign_game(game_id);
                rollup_manager.is_creator = lobby.mode == LobbyMode::Create;
                rollup_manager.used_global_session = lobby.last_attempt_used_global_session;
                info!(
                    "[LOBBY] Active game_id {} stored globally (rollup updated, is_creator={}, used_global_session={})",
                    game_id, rollup_manager.is_creator, rollup_manager.used_global_session
                );

                if lobby.mode == LobbyMode::Create {
                    // Announce all on-chain games through the P2P relay; zero-stake games use
                    // the P2P tag for the ordinary browse list.
                    let is_wagered = lobby.wager_sol > 0.0;
                    let game_type = if is_wagered { "solana_wager" } else { "P2P" };
                    let display_name = lobby
                        .cached_display_name
                        .clone()
                        .unwrap_or_else(|| "Anonymous".to_string());
                    let host_node_id = lobby
                        .cached_node_id
                        .clone()
                        .unwrap_or_else(|| "unknown_node_id".to_string());

                    let announce_result = if let Some(ref pwd) = lobby.room_password.clone() {
                        crate::multiplayer::vps_client::p2p_announce_game_with_password(
                            game_id.to_string(),
                            &host_node_id,
                            &display_name,
                            lobby.wager_sol as f64,
                            game_type,
                            lobby.time_control_base,
                            lobby.time_control_inc as u16,
                            Some(display_name.clone()),
                            if lobby.cached_elo > 0 {
                                Some(lobby.cached_elo)
                            } else {
                                None
                            },
                            lobby.cached_region.clone(),
                            pwd.clone(),
                        )
                    } else {
                        crate::multiplayer::vps_client::p2p_announce_game(
                            game_id.to_string(),
                            &host_node_id,
                            &display_name,
                            lobby.wager_sol as f64,
                            game_type,
                            lobby.time_control_base,
                            lobby.time_control_inc as u16,
                            Some(display_name.clone()),
                            if lobby.cached_elo > 0 {
                                Some(lobby.cached_elo)
                            } else {
                                None
                            },
                            lobby.cached_region.clone(),
                        )
                    };
                    if let Err(e) = announce_result {
                        warn!("[LOBBY] Failed to announce game {} to VPS: {}", game_id, e);
                        lobby.announce_warning = Some(format!(
                            "Couldn't list this game publicly ({e}) — share the Game ID directly instead."
                        ));
                    } else {
                        info!(
                            "[LOBBY] Announced game {} ({}) to VPS relay",
                            game_id, game_type
                        );

                        // Register the host with the relay so JOIN_ACK supplies the joining node ID
                        // and establishes transport even when on-chain join succeeds first.
                        p2p_vps.hosting_game_id = Some(game_id.to_string());
                        p2p_vps.hosting_node_id = Some(host_node_id.clone());
                        p2p_vps.hosting_stake_amount = lobby.wager_sol as f64;
                        p2p_vps.hosting_base_secs = lobby.time_control_base;
                        p2p_vps.hosting_inc = lobby.time_control_inc as u16;
                        p2p_vps.host_poll_last = None; // poll immediately
                    }
                }
            }
            Ok(Err(e)) => {
                lobby.status = LobbyStatus::Error(e);
                lobby.tx_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.status = LobbyStatus::Error("Task dropped".to_string());
                lobby.tx_rx = None;
            }
        }
    }

    // Poll opponent-joined receiver.
    if let Some(ref mut rx) = lobby.opponent_poll_rx {
        match rx.try_recv() {
            Ok(Ok(())) => {
                let game_id = match lobby.status {
                    LobbyStatus::WaitingForOpponent { game_id } => game_id,
                    _ => 0,
                };
                lobby.status = LobbyStatus::OpponentJoined { game_id };
                lobby.opponent_poll_rx = None;
            }
            Ok(Err(e)) => {
                lobby.status = LobbyStatus::Error(e);
                lobby.opponent_poll_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.status = LobbyStatus::Error("Poll task dropped".to_string());
                lobby.opponent_poll_rx = None;
            }
        }
    }

    // Poll for the host's GAME_START signal (joiner side) — see
    // `WaitingForHostStart` in `screens.rs`'s auto-transition.
    if let Some(ref mut rx) = lobby.game_start_poll_rx {
        match rx.try_recv() {
            Ok(Ok(())) => {
                let game_id = match lobby.status {
                    LobbyStatus::WaitingForHostStart { game_id } => game_id,
                    _ => 0,
                };
                lobby.status = LobbyStatus::EnterGame { game_id };
                lobby.game_start_poll_rx = None;
            }
            Ok(Err(e)) => {
                lobby.status = LobbyStatus::Error(e);
                lobby.game_start_poll_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.status = LobbyStatus::Error("Game-start poll task dropped".to_string());
                lobby.game_start_poll_rx = None;
            }
        }
    }

    // Poll lookup receiver.
    if let Some(ref mut rx) = lobby.lookup_rx {
        match rx.try_recv() {
            Ok(Ok((wager_lamports, game_id))) => {
                let wager_sol = wager_lamports as f64 / 1_000_000_000.0;
                lobby.status = LobbyStatus::Fetched { wager_sol, game_id };
                lobby.lookup_rx = None;
            }
            Ok(Err(e)) => {
                lobby.status = LobbyStatus::Error(e);
                lobby.lookup_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.status = LobbyStatus::Error("Lookup dropped".to_string());
                lobby.lookup_rx = None;
            }
        }
    }
}

pub(super) fn sync_from_solana_state(
    solana: Res<crate::multiplayer::solana::integration::SolanaIntegrationState>,
    mut lobby: ResMut<SolanaLobbyState>,
    region: Res<crate::multiplayer::social::BackendRegion>,
) {
    lobby.cached_balance = solana.balance;
    lobby.cached_rpc_url = DEVNET_RPC_URL.to_string();
    lobby.cached_elo = solana.cached_elo;
    // This field is the game-creation enforcement point for disabling cached
    // global-session keys, regardless of wallet-level session state.
    let _ = &solana.global_session_active;
    let _ = &solana.global_session_keypair;
    lobby.cached_global_session_keypair_bytes = None;
    if !region.tag.is_empty() {
        lobby.cached_region = Some(region.tag.clone());
    }
    if lobby.cached_display_name.is_none() {
        lobby.cached_display_name = solana.cached_display_name.clone();
    }

    let wallet_bytes = solana
        .wallet_pubkey
        .map(|pubkey| pubkey.to_bytes().to_vec());
    if lobby.cached_keypair_bytes != wallet_bytes {
        // A lookup from the previous wallet must not offer its game to the
        // newly selected wallet, even if that lookup finishes late.
        lobby.cached_keypair_bytes = wallet_bytes;
        lobby.rejoin_rx = None;
        lobby.rejoin_game_id = None;
        lobby.rejoin_lookup_error = None;
        lobby.recovery_rx = None;
        lobby.recovery_scan = None;
        lobby.recovery_action_rx = None;
        lobby.recovery_message = None;
        lobby.recovery_refresh_at = None;
        lobby.cached_display_name = None;
        if let Some(pubkey) = solana.wallet_pubkey {
            let (tx, rx) = oneshot::channel();
            spawn_check_active_game(pubkey, tx);
            lobby.rejoin_rx = Some(rx);
            lobby.start_recovery_scan(pubkey);
        }
    }
}

pub(super) fn poll_recovery_scan(mut lobby: ResMut<SolanaLobbyState>) {
    if let Some(rx) = lobby.recovery_action_rx.as_mut() {
        let result = match rx.try_recv() {
            Ok(result) => Some(result),
            Err(oneshot::error::TryRecvError::Empty) => None,
            Err(_) => Some(Err(
                "Settlement check stopped. Check the chain again before retrying.".into(),
            )),
        };
        if let Some(result) = result {
            let succeeded = result.is_ok();
            lobby.recovery_message = Some(result);
            lobby.recovery_action_rx = None;
            lobby.rescan_recovery();
            if succeeded {
                lobby.recovery_refresh_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(35));
            }
        }
    }
    if lobby
        .recovery_refresh_at
        .is_some_and(|at| std::time::Instant::now() >= at)
        && lobby.recovery_rx.is_none()
    {
        lobby.recovery_refresh_at = None;
        lobby.rescan_recovery();
    }
    if let Some(ref mut rx) = lobby.recovery_rx {
        match rx.try_recv() {
            Ok(scan) => {
                lobby.recovery_scan = Some(scan);
                lobby.recovery_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.recovery_scan = Some(crate::multiplayer::solana::wager_recovery::WagerScan {
                    complete: false,
                    errors: vec!["stake recovery scan stopped unexpectedly".to_string()],
                    ..Default::default()
                });
                lobby.recovery_rx = None;
            }
        }
    }
}

pub fn spawn_check_active_game(
    wallet_pubkey: Pubkey,
    tx: oneshot::Sender<Result<Option<u64>, String>>,
) {
    crate::core::net_task::spawn_detached("lobby", move || {
        let result =
            crate::multiplayer::vps_client::get_active_game_for_wallet(&wallet_pubkey.to_string());
        let _ = tx.send(result);
    });
}

pub(super) fn poll_rejoin_check(mut lobby: ResMut<SolanaLobbyState>) {
    if let Some(ref mut rx) = lobby.rejoin_rx {
        match rx.try_recv() {
            Ok(Ok(maybe_id)) => {
                lobby.rejoin_game_id = maybe_id;
                lobby.rejoin_lookup_error = None;
                lobby.rejoin_rx = None;
            }
            Ok(Err(e)) => {
                lobby.rejoin_lookup_error = Some(e);
                lobby.rejoin_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                lobby.rejoin_lookup_error =
                    Some("Active-game lookup stopped unexpectedly".to_string());
                lobby.rejoin_rx = None;
            }
        }
    }
}

pub fn poll_tournament_games(mut lobby: ResMut<SolanaLobbyState>) {
    // Drain result if pending.
    if let Some(ref rx) = lobby.tournament_rx {
        if let Ok(games) = rx.try_recv() {
            lobby.tournament_games = games;
            lobby.tournament_rx = None;
        }
    }

    if lobby.mode != LobbyMode::Tournament {
        return;
    }
    let should_refresh = lobby
        .tournament_last_fetch
        .map(|t| t.elapsed().as_secs() >= 10)
        .unwrap_or(true);
    if !should_refresh || lobby.tournament_rx.is_some() {
        return;
    }
    lobby.tournament_last_fetch = Some(std::time::Instant::now());

    let (tx, rx) = crossbeam_channel::bounded(1);
    lobby.tournament_rx = Some(rx);
    crate::core::net_task::spawn_detached("tournament-games", move || {
        match crate::multiplayer::network::vps::list_tournament_games() {
            Ok(games) => {
                let _ = tx.send(games);
            }
            Err(e) => warn!("[SOLANA_TOURNEY] Failed to fetch tournament games: {}", e),
        }
    });
}

pub fn poll_solana_browse(
    mut lobby: ResMut<SolanaLobbyState>,
    snapshot: Res<crate::multiplayer::network::p2p_vps::P2PVpsState>,
) {
    if lobby.mode != LobbyMode::Browse
        || lobby.browse_revision == Some(snapshot.cached_games_revision)
    {
        return;
    }
    lobby.browse_games = snapshot
        .cached_games
        .iter()
        .filter(|g| g.game_type == "solana_wager" || g.game_type == "P2P")
        .cloned()
        .collect();
    lobby.browse_revision = Some(snapshot.cached_games_revision);
}
