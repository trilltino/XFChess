use super::*;

#[derive(Debug, Default)]
pub(super) struct FinalizationResult {
    pub(super) sig: String,
    pub(super) winner_lamports: u64,
    pub(super) country_fee: u64,
    pub(super) operating_cost_lamports: u64,
    pub(super) elo_fee: u64,
    /// Set only from chain evidence, never from the HTTP response alone.
    pub(super) status: SettlementStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SettlementStatus {
    /// The finalize signature confirmed, or the Game PDA is closed (which
    /// only `finalize_game` does, after paying out the escrow).
    Confirmed,
    /// Not proven on chain; the settlement worker keeps retrying server-side.
    Pending(String),
}

impl Default for SettlementStatus {
    fn default() -> Self {
        Self::Pending("Settlement not yet confirmed on chain".to_string())
    }
}

/// Combine signature status and Game PDA existence into settlement status.
/// None means an unknown signature result or failed account lookup.
pub(super) fn settlement_status(
    sig_status: Option<Result<(), String>>,
    game_closed: Option<bool>,
) -> SettlementStatus {
    match (sig_status, game_closed) {
        (Some(Ok(())), _) | (_, Some(true)) => SettlementStatus::Confirmed,
        (Some(Err(e)), _) => SettlementStatus::Pending(format!(
            "Settlement transaction failed ({e}); it will be retried automatically"
        )),
        (None, _) => SettlementStatus::Pending(
            "Settlement submitted; waiting for on-chain confirmation".to_string(),
        ),
    }
}

/// Poll the finalize signature, then fall back to the Game PDA's existence.
pub(super) fn verify_settlement_on_chain(
    rpc: &RpcClient,
    game_pda: &Pubkey,
    sig: Option<&str>,
) -> SettlementStatus {
    let mut sig_status = None;
    if let Some(sig) = sig.and_then(|s| s.parse::<solana_sdk::signature::Signature>().ok()) {
        for _ in 0..20 {
            match rpc.get_signature_status_with_commitment(&sig, CommitmentConfig::confirmed()) {
                Ok(Some(Ok(()))) => {
                    sig_status = Some(Ok(()));
                    break;
                }
                Ok(Some(Err(e))) => {
                    sig_status = Some(Err(format!("{e:?}")));
                    break;
                }
                Ok(None) | Err(_) => std::thread::sleep(std::time::Duration::from_millis(500)),
            }
        }
    }
    let game_closed = if sig_status == Some(Ok(())) {
        None
    } else {
        rpc.get_account_with_commitment(game_pda, CommitmentConfig::confirmed())
            .ok()
            .map(|resp| resp.value.is_none())
    };
    settlement_status(sig_status, game_closed)
}

pub(super) const MAX_UNDELEGATE_WAIT_SECS: u64 = 60;

pub(super) const MAX_FINALIZATION_WAIT_FRAMES: u32 = 600;

#[derive(Debug)]
pub(super) struct PendingFinalization {
    pub(super) game_id: u64,
    pub(super) winner: Option<String>,
    pub(super) local_pk: Pubkey,
    pub(super) is_creator: bool,
    pub(super) frames_waited: u32,
    pub(super) wager_lamports: u64,
}

pub(super) fn poll_game_end_flush(mut bridge: ResMut<RollupNetworkBridge>) {
    if let Some(ref mut rx) = bridge.game_end_flush_rx {
        match rx.try_recv() {
            Ok(()) | Err(oneshot::error::TryRecvError::Closed) => {
                bridge.game_end_moves_flushing = false;
                bridge.game_end_flush_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                // Still flushing, nothing to do.
            }
        }
    }
}

pub(super) fn handle_game_end_causal_cleanup(
    mut game_ended_events: MessageReader<GameEndedEvent>,
    mut causal: ResMut<crate::multiplayer::types::CausalChainState>,
) {
    for event in game_ended_events.read() {
        let game_id = event.game_id;
        causal.last_seq.retain(|(gid, _), _| *gid != game_id);
        causal.head_version.retain(|(gid, _), _| *gid != game_id);
        causal.roster.remove(&game_id);
    }
}

pub(super) fn handle_game_end_undelegation(
    mut game_ended_events: MessageReader<GameEndedEvent>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    rollup_manager: Res<EphemeralRollupManager>,
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
    mut bridge: ResMut<RollupNetworkBridge>,
) {
    for event in game_ended_events.read() {
        // Only the host drives settlement; the backend worker recovers if it dies mid-flow.
        if !rollup_manager.is_creator {
            continue;
        }
        // Use the Solana on-chain game_id (rollup_manager), not the P2P event ID.
        let game_id = if rollup_manager.game_id != 0 {
            rollup_manager.game_id
        } else {
            event.game_id
        };

        info!(
            "[FINALIZE] Game {} ended (winner={:?} reason={}) — preparing on-chain finalization",
            game_id, event.winner, event.reason
        );

        // Derive and log the move_log PDA so the user can look up moves on Solscan.
        let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
        let move_log_pda = solana_sdk::pubkey::Pubkey::find_program_address(
            &[b"move_log", &game_id.to_le_bytes()],
            &program_id,
        )
        .0;
        info!("[FINALIZE] move_log PDA: {}", move_log_pda);
        info!(
            "[FINALIZE] Solscan: https://solscan.io/account/{}?cluster=devnet",
            move_log_pda
        );

        let is_delegated = magicblock_resolver.is_delegated();
        let game_pda = magicblock_resolver.get_delegated_game().unwrap_or_default();

        // Resolve white/black wallet pubkeys.
        // is_creator ↔ white; joiner ↔ black.
        let (white_pk, black_pk) = match solana_state.as_ref() {
            Some(s) => {
                let local = s.wallet_pubkey.unwrap_or_default();
                let opponent = s.opponent_pubkey.unwrap_or_default();
                if rollup_manager.is_creator {
                    (local, opponent)
                } else {
                    (opponent, local)
                }
            }
            None => {
                warn!(
                    "[FINALIZE] No wallet state — cannot finalize game {}",
                    game_id
                );
                if is_delegated {
                    magicblock_events.write(MagicBlockEvent::UndelegationFailed {
                        game_pda,
                        error: "no wallet state for finalization".to_string(),
                    });
                }
                continue;
            }
        };

        if white_pk == Pubkey::default() || black_pk == Pubkey::default() {
            warn!(
                "[FINALIZE] Opponent pubkey unavailable for game {} — deferring finalization",
                game_id
            );
            let local_pk = solana_state
                .as_ref()
                .and_then(|s| s.wallet_pubkey)
                .unwrap_or_default();
            let wager = competitive.as_ref().map(|c| c.stake_amount).unwrap_or(0);
            bridge.pending_finalization = Some(PendingFinalization {
                game_id,
                winner: event.winner.clone(),
                local_pk,
                is_creator: rollup_manager.is_creator,
                frames_waited: 0,
                wager_lamports: wager,
            });
            continue;
        }

        let winner = event.winner.clone();

        // Item 4: Free Rated path — game was never delegated, so just update ELO.
        if !is_delegated {
            let w = white_pk.to_string();
            let b = black_pk.to_string();
            let win = winner.clone();
            crate::core::net_task::spawn_detached("bridge", move || {
                use crate::multiplayer::vps_client;
                if let Err(e) =
                    vps_client::vps_submit_free_rated_result(game_id, win.as_deref(), &w, &b)
                {
                    error!("[FREE_RATED] ELO update failed for game {}: {e}", game_id);
                } else {
                    info!("[FREE_RATED] ELO updated for game {}", game_id);
                }
            });
            continue;
        }

        let wager = competitive.as_ref().map(|c| c.wager_lamports).unwrap_or(0);

        // Defer finalization at least one frame so batch-flush systems have run
        // before checking game_end_moves_flushing.
        let local_pk = if rollup_manager.is_creator {
            white_pk
        } else {
            black_pk
        };
        bridge.pending_finalization = Some(PendingFinalization {
            game_id,
            winner,
            local_pk,
            is_creator: rollup_manager.is_creator,
            frames_waited: 0,
            wager_lamports: wager,
        });
    }
}

pub(super) fn spawn_finalization_task(
    game_id: u64,
    winner: Option<String>,
    white_pk: Pubkey,
    black_pk: Pubkey,
    wager_lamports: u64,
    result_tx: oneshot::Sender<FinalizationResult>,
    fallback_er_endpoint: String,
) {
    crate::core::net_task::spawn_detached("bridge", move || {
        use crate::multiplayer::rollup::magicblock::er_explorer_url_for;
        use crate::multiplayer::solana::integration::state::DEVNET_RPC_URL;
        use crate::multiplayer::vps_client;
        use crate::solana::instructions::PROGRAM_ID as SOLANA_PROGRAM_ID;
        use solana_client::rpc_client::RpcClient;
        use solana_commitment_config::CommitmentConfig;

        // Allow ER move commits to settle before undelegation; these paths have no
        // shared completion signal. Shorter delays require live verification.
        std::thread::sleep(std::time::Duration::from_secs(2));

        match vps_client::vps_undelegate_game(game_id) {
            Ok((sig, resp_endpoint)) => {
                let endpoint = if resp_endpoint.is_empty() {
                    fallback_er_endpoint.as_str()
                } else {
                    resp_endpoint.as_str()
                };
                info!(
                    "[UNDELEGATE] ER committed for game {} sig {} — inspect: {}",
                    game_id,
                    sig,
                    er_explorer_url_for(endpoint, &sig)
                )
            }
            Err(e) => error!(
                "[UNDELEGATE] Failed for game {}: {e} — continuing to finalize",
                game_id
            ),
        }

        // Item 2: Poll devnet until game PDA owner returns to the program (not ER).
        let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
        let game_pda =
            Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;
        let rpc = RpcClient::new_with_commitment(
            DEVNET_RPC_URL.to_string(),
            CommitmentConfig::confirmed(),
        );
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(MAX_UNDELEGATE_WAIT_SECS);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            match rpc.get_account(&game_pda) {
                Ok(acc) if acc.owner == program_id => {
                    info!(
                        "[UNDELEGATE] Game {} PDA returned to devnet — proceeding to finalize",
                        game_id
                    );
                    break;
                }
                Ok(_) => {}  // still owned by ER
                Err(_) => {} // transient RPC error — keep polling
            }
            if std::time::Instant::now() >= deadline {
                warn!(
                    "[UNDELEGATE] Game {} PDA did not return after {}s — finalizing anyway",
                    game_id, MAX_UNDELEGATE_WAIT_SECS
                );
                break;
            }
        }

        let w_str = white_pk.to_string();
        let b_str = black_pk.to_string();
        let win_ref = winner.as_deref();

        match vps_client::vps_finalize_game(game_id, win_ref, &w_str, &b_str, wager_lamports) {
            Ok(result) => {
                let status = verify_settlement_on_chain(&rpc, &game_pda, Some(&result.sig));
                info!(
                        "[FINALIZED] Game {} finalize returned payout {} lamports, sig {}, chain status {:?}",
                        game_id, result.winner_lamports, result.sig, status
                    );
                if result.country_fee > 0 {
                    info!(
                        "[TREASURY] Game {} platform fee: {} lamports paid to treasury_vault",
                        game_id, result.country_fee
                    );
                }
                let _ = result_tx.send(FinalizationResult {
                    sig: result.sig,
                    winner_lamports: result.winner_lamports,
                    country_fee: result.country_fee,
                    operating_cost_lamports: result.operating_cost_lamports,
                    elo_fee: result.elo_fee,
                    status,
                });
            }
            Err(e) => {
                error!("[FINALIZE] Game {} finalization failed: {e}", game_id);
                // A failed request may still have settled; the worker retries server-side.
                // Report the observed chain state without claiming an unconfirmed payout.
                let status = match verify_settlement_on_chain(&rpc, &game_pda, None) {
                    SettlementStatus::Confirmed => SettlementStatus::Confirmed,
                    SettlementStatus::Pending(_) => SettlementStatus::Pending(
                        "Settlement has not completed yet; it will be retried automatically"
                            .to_string(),
                    ),
                };
                let _ = result_tx.send(FinalizationResult {
                    status,
                    ..Default::default()
                });
            }
        }
    });
}

pub(super) fn retry_pending_finalization(
    mut bridge: ResMut<RollupNetworkBridge>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    magicblock_resolver: Res<MagicBlockResolver>,
) {
    let pending = match bridge.pending_finalization.take() {
        Some(p) => p,
        None => return,
    };

    let opponent_pk = solana_state.as_ref().and_then(|s| s.opponent_pubkey);
    let (white_pk, black_pk) = match opponent_pk {
        Some(opp) => {
            if pending.is_creator {
                (pending.local_pk, opp)
            } else {
                (opp, pending.local_pk)
            }
        }
        None => {
            let new_frames = pending.frames_waited + 1;
            if new_frames > MAX_FINALIZATION_WAIT_FRAMES {
                warn!(
                    "[FINALIZE] Opponent pubkey not received after {} frames for game {} — giving up",
                    new_frames, pending.game_id
                );
                return;
            }
            bridge.pending_finalization = Some(PendingFinalization {
                frames_waited: new_frames,
                ..pending
            });
            return;
        }
    };

    if white_pk == Pubkey::default() || black_pk == Pubkey::default() {
        warn!(
            "[FINALIZE] Resolved pubkeys still default for game {} — skipping",
            pending.game_id
        );
        return;
    }

    // Wait for the final move batch before undelegating; share the existing frame timeout.
    if bridge.game_end_moves_flushing {
        let new_frames = pending.frames_waited + 1;
        if new_frames > MAX_FINALIZATION_WAIT_FRAMES {
            warn!(
                "[FINALIZE] Move batch still flushing after {} frames for game {} — finalizing anyway",
                new_frames, pending.game_id
            );
        } else {
            bridge.pending_finalization = Some(PendingFinalization {
                frames_waited: new_frames,
                ..pending
            });
            return;
        }
    }

    info!(
        "[FINALIZE] Opponent pubkey arrived after {} frames for game {} — finalizing",
        pending.frames_waited, pending.game_id
    );
    let (fin_tx, fin_rx) = oneshot::channel::<FinalizationResult>();
    bridge.finalization_rx = Some(fin_rx);
    spawn_finalization_task(
        pending.game_id,
        pending.winner,
        white_pk,
        black_pk,
        pending.wager_lamports,
        fin_tx,
        magicblock_resolver.er_endpoint().to_string(),
    );
}

pub(super) fn apply_finalization_result(
    mut bridge: ResMut<RollupNetworkBridge>,
    mut payout_info: Option<ResMut<GameOverPayoutInfo>>,
) {
    let rx = match bridge.finalization_rx.as_mut() {
        Some(rx) => rx,
        None => return,
    };
    match rx.try_recv() {
        Ok(result) => {
            bridge.finalization_rx = None;
            if let Some(ref mut info) = payout_info {
                if let SettlementStatus::Pending(reason) = result.status {
                    // Keep the popup in its "settling" state with the reason;
                    // the HTTP response alone never marks a payout complete.
                    warn!("[FINALIZE] Settlement not confirmed on chain: {reason}");
                    info.settlement_pending_reason = Some(reason);
                    if !result.sig.is_empty() {
                        info.finalize_sig = Some(result.sig);
                    }
                    return;
                }
                info.payout_confirmed = true;
                info.settlement_pending_reason = None;
                info.finalize_sig = (!result.sig.is_empty()).then_some(result.sig);
                if result.winner_lamports > 0 {
                    info.winning_prize = result.winner_lamports;
                }
                // Overwrite the estimate even for zero: a confirmed draw or free game has no payout.
                info.country_fee = result.country_fee;
                info.elo_fee = result.elo_fee;
                info.operating_cost = result.operating_cost_lamports;
                info.fee_breakdown_confirmed = true;
                info.game_ended_at = Some(std::time::Instant::now());
            }
        }
        Err(oneshot::error::TryRecvError::Empty) => {}
        Err(_) => {
            bridge.finalization_rx = None;
        }
    }
}

pub(super) fn apply_nonce_resync(mut bridge: ResMut<RollupNetworkBridge>) {
    let rx = match bridge.nonce_rx.as_mut() {
        Some(rx) => rx,
        None => return,
    };
    match rx.try_recv() {
        Ok(next_nonce) => {
            bridge.move_nonce = next_nonce;
            bridge.nonce_rx = None;
            info!("[NONCE] Local move_nonce set to {}", next_nonce);
        }
        Err(oneshot::error::TryRecvError::Empty) => {}
        Err(_) => {
            bridge.nonce_rx = None;
        }
    }
}

#[cfg(test)]
mod game_end_ordering_tests {
    use super::*;
    use crate::game::events::GameEndedEvent;
    use crate::multiplayer::rollup::magicblock::DelegationStatus;
    use crate::multiplayer::solana::addon::CompetitiveMatchState;
    use crate::multiplayer::solana::integration::state::SolanaIntegrationState;
    use crate::multiplayer::systems::finalize_game_on_end;
    use crate::multiplayer::types::OnlineNetworkState;
    use bevy::prelude::MinimalPlugins;

    fn build_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        // Register only the ordered systems under test. The full plugin requires
        // unrelated resources that would fail Bevy system validation.
        app.add_message::<GameEndedEvent>();
        app.add_message::<RollupEvent>();
        app.add_message::<MagicBlockEvent>();
        app.insert_resource(RollupNetworkBridge::new());
        app.insert_resource(MagicBlockResolver::default());
        app.init_resource::<RecentTransactions>();

        app.add_systems(
            Update,
            (
                finalize_game_on_end,
                handle_rollup_to_network_events.after(finalize_game_on_end),
                handle_game_end_undelegation,
                retry_pending_finalization.after(handle_rollup_to_network_events),
                poll_game_end_flush,
            ),
        );

        // Queue a move so force_flush emits GameEndBatch and the test exercises the flushing guard.
        let mut mgr = EphemeralRollupManager::new(777, true, "startpos".to_string());
        mgr.add_local_move("g2g4".to_string(), "fen_after_g2g4".to_string());
        app.insert_resource(mgr);

        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        app.insert_resource(SolanaIntegrationState {
            wallet_pubkey: Some(white),
            opponent_pubkey: Some(black),
            ..Default::default()
        });
        app.insert_resource(CompetitiveMatchState {
            wager_lamports: 1_000_000,
            ..Default::default()
        });
        app.insert_resource(OnlineNetworkState::default());

        // Use a delegated Game PDA so the test reaches the undelegation path.
        {
            let mut resolver = app.world_mut().resource_mut::<MagicBlockResolver>();
            resolver.delegation_status = DelegationStatus::Delegated;
            resolver.delegated_game_pda = Some(Pubkey::new_unique());
        }

        app
    }

    #[test]
    fn finalize_never_fires_while_game_end_batch_still_flushing() {
        let mut app = build_test_app();

        app.world_mut().write_message(GameEndedEvent {
            game_id: 777,
            winner: Some("black".to_string()),
            reason: "checkmate".to_string(),
        });

        // The chained systems must set the flushing flag before retry within this update.
        app.update();
        let bridge = app.world().resource::<RollupNetworkBridge>();
        assert!(
            bridge.game_end_moves_flushing,
            "expected the move batch to be flushing after the first frame — \
             test setup problem, not the bug under test, if this fails"
        );
        assert!(
            bridge.finalization_rx.is_none(),
            "REGRESSION: finalize/undelegate fired while the game-end move \
             batch was still flushing — this is the exact race that \
             stranded a real-wager game live on 2026-08-11"
        );

        // The task is still pending on frame 2, so undelegation must remain blocked.
        app.update();
        let bridge = app.world().resource::<RollupNetworkBridge>();
        assert!(
            bridge.finalization_rx.is_none(),
            "REGRESSION: finalize/undelegate fired on a later frame while \
             still flushing"
        );
    }
}

#[cfg(test)]
mod settlement_status_tests {
    use super::*;

    #[test]
    fn confirmed_signature_or_closed_game_proves_settlement() {
        assert_eq!(
            settlement_status(Some(Ok(())), None),
            SettlementStatus::Confirmed
        );
        // finalize_game closes the PDA, so closure is proof even when this
        // client's own request failed (e.g. the settlement worker won).
        assert_eq!(
            settlement_status(None, Some(true)),
            SettlementStatus::Confirmed
        );
        assert_eq!(
            settlement_status(Some(Err("x".into())), Some(true)),
            SettlementStatus::Confirmed
        );
    }

    #[test]
    fn http_success_without_chain_evidence_stays_pending() {
        assert!(matches!(
            settlement_status(None, Some(false)),
            SettlementStatus::Pending(_)
        ));
        // RPC lookup failure is not evidence either way.
        assert!(matches!(
            settlement_status(None, None),
            SettlementStatus::Pending(_)
        ));
        assert!(matches!(
            settlement_status(Some(Err("custom program error".into())), Some(false)),
            SettlementStatus::Pending(_)
        ));
        assert!(matches!(
            FinalizationResult::default().status,
            SettlementStatus::Pending(_)
        ));
    }
}
