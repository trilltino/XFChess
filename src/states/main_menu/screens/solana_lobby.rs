use super::*;

#[cfg(feature = "solana")]
pub(in super::super) fn ui_solana_lobby(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    let Some(mut lobby) = ctx.solana_lobby.take() else {
        ui.label("Solana lobby not available.");
        return;
    };

    lobby.cached_display_name = Some(ctx.player_identity.display_name().to_string());
    lobby.cached_node_id = ctx.network_state.as_ref().and_then(|ns| {
        ns.node_id
            .map(|id| bs58::encode(id.as_bytes()).into_string())
    });
    lobby.cached_secret_key_bytes = ctx
        .network_state
        .as_ref()
        .and_then(|ns| ns.secret_key_bytes);

    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        let active_game_id = match lobby.status {
            LobbyStatus::WaitingForOpponent { game_id }
            | LobbyStatus::OpponentJoined { game_id }
            | LobbyStatus::WaitingForHostStart { game_id }
            | LobbyStatus::CancelFailed { game_id, .. } => Some(game_id),
            _ => None,
        };
        if ui.button("‹ Back").clicked() {
            if let Some(game_id) = active_game_id {
                begin_solana_lobby_cancel(ctx, &mut lobby, game_id);
            } else if matches!(lobby.status, LobbyStatus::Pending) {
                // Wait for the create/join result so late confirmation cannot create an untracked lobby.
            } else if !matches!(lobby.status, LobbyStatus::Cancelling { .. }) {
                ctx.menu_state.set(crate::core::MenuState::ModeSelect);
                lobby.status = LobbyStatus::Idle;
            }
        }

        Layout::section_space(ui);

        ui.label(
            egui::RichText::new("SOLANA WAGER LOBBY")
                .size(24.0)
                .color(egui::Color32::from_rgb(255, 150, 100))
                .strong(),
        );

        // Resume only from verified chain state and move history, retaining board and nonce.
        if let Some(rejoin_id) = lobby.rejoin_game_id {
            use crate::multiplayer::resume::ResumeStatus;
            let wallet = ctx.solana_state.as_ref().and_then(|s| s.wallet_pubkey);
            let rpc_url = lobby.cached_rpc_url.clone();
            ui.separator();
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 220, 50),
                    format!("Game {rejoin_id} is still in progress. Its clock keeps running."),
                );
                if let Some(resume) = ctx.resume.as_mut() {
                    if resume.is_checking() {
                        ui.spinner();
                        ui.label("Verifying game…");
                    } else if let Some(wallet) = wallet {
                        if ui.button("Resume game").clicked() {
                            resume.request(
                                rejoin_id,
                                wallet,
                                rpc_url.clone(),
                                crate::multiplayer::rollup::magicblock::MAGIC_BLOCK_ER_ENDPOINT
                                    .to_string(),
                            );
                        }
                    }
                }
                if ui.small_button("Dismiss").clicked() {
                    lobby.rejoin_game_id = None;
                }
            });
            if let Some(ResumeStatus::Failed { game_id, message }) =
                ctx.resume.as_ref().map(|r| r.status.clone())
            {
                if game_id == rejoin_id {
                    ui.colored_label(egui::Color32::from_rgb(255, 160, 120), message);
                }
            }
            ui.separator();
        }
        if lobby.rejoin_lookup_error.is_some() {
            ui.colored_label(
                egui::Color32::from_rgb(255, 200, 100),
                "Could not check for an existing game. Try again when connected.",
            );
        }
        render_stake_recovery(ui, ctx, &mut lobby);

        Layout::item_space(ui);

        // Display USD when a live rate is available; otherwise show SOL.
        let balance = lobby.cached_balance;
        let wallet_ready = lobby.cached_keypair_bytes.is_some();
        if wallet_ready {
            let usd = ctx
                .sol_usd_wager_rate
                .as_ref()
                .and_then(|rate| rate.usd_for_sol(balance));
            let balance_text = match usd {
                Some(usd) => format!("Wallet balance: ${:.2}", usd),
                None => format!("Wallet balance: {:.4} SOL", balance),
            };
            ui.label(egui::RichText::new(balance_text).color(egui::Color32::GOLD));
        } else {
            ui.colored_label(egui::Color32::YELLOW, "Wallet not connected");
            if ui.button("Connect Wallet").clicked() {
                tauri_signer::open_wallet_browser();
            }
        }

        Layout::item_space(ui);

        // Hide forms for post-transaction Create states only while Create is active;
        // stale Create status must not hide Join or Browse.
        let in_post_state = lobby.mode == LobbyMode::Create
            && matches!(
                lobby.status,
                LobbyStatus::WaitingForOpponent { .. }
                    | LobbyStatus::OpponentJoined { .. }
                    | LobbyStatus::Success(_)
            );

        let usd_per_sol = ctx
            .sol_usd_wager_rate
            .as_ref()
            .and_then(|r| r.current.as_ref())
            .map(|s| s.usd_per_sol);

        if !in_post_state {
            // Entered via "Find Wagered Game" ⇒ join/browse only; never let the
            // hidden Create tab stay active.
            if !lobby.allow_create && lobby.mode == LobbyMode::Create {
                lobby.mode = LobbyMode::Browse;
                lobby.browse_last_fetch = None;
            }

            // Tab switcher
            ui.horizontal(|ui| {
                if lobby.allow_create
                    && ui
                        .selectable_label(lobby.mode == LobbyMode::Create, "Create Game")
                        .clicked()
                {
                    lobby.mode = LobbyMode::Create;
                    lobby.status = LobbyStatus::Idle;
                }
                if ui
                    .selectable_label(lobby.mode == LobbyMode::Join, "Join by ID")
                    .clicked()
                {
                    lobby.mode = LobbyMode::Join;
                    lobby.status = LobbyStatus::Idle;
                }
                if ui
                    .selectable_label(lobby.mode == LobbyMode::Browse, "Browse Games")
                    .clicked()
                {
                    lobby.mode = LobbyMode::Browse;
                    lobby.status = LobbyStatus::Idle;
                    lobby.browse_last_fetch = None; // trigger immediate refresh
                }
                if ui
                    .selectable_label(lobby.mode == LobbyMode::Tournament, "Tournament")
                    .clicked()
                {
                    lobby.mode = LobbyMode::Tournament;
                    lobby.status = LobbyStatus::Idle;
                    lobby.tournament_last_fetch = None; // trigger immediate refresh
                }
            });

            ui.separator();
            Layout::item_space(ui);

            let node_id_b58 = ctx.network_state.as_ref().and_then(|ns| {
                ns.node_id
                    .map(|id| bs58::encode(id.as_bytes()).into_string())
            });
            let secret_key_bytes = ctx.network_state.as_ref().and_then(|ns| ns.secret_key_bytes);
            let global_session_setup_in_progress = ctx
                .solana_state
                .as_ref()
                .map(|s| s.global_session_setup_in_progress)
                .unwrap_or(false);
            let global_session_unavailable_reason = ctx
                .solana_state
                .as_ref()
                .and_then(|s| s.global_session_unavailable_reason.as_deref());
            match lobby.mode {
                LobbyMode::Create => render_create_tab(
                    ui,
                    &mut lobby,
                    &mut ctx.compliance,
                    node_id_b58.as_deref(),
                    usd_per_sol,
                    global_session_setup_in_progress,
                    global_session_unavailable_reason,
                    ctx.solana_session.as_deref(),
                ),
                LobbyMode::Join => render_join_tab(
                    ui,
                    &mut lobby,
                    node_id_b58.as_deref(),
                    secret_key_bytes,
                    usd_per_sol,
                    global_session_setup_in_progress,
                    global_session_unavailable_reason,
                    ctx.solana_session.as_deref(),
                ),
                LobbyMode::Browse => render_solana_browse_tab(ui, &mut lobby, usd_per_sol),
                LobbyMode::Tournament => {
                    render_solana_tournament_tab(ui, &mut lobby, &mut ctx.spectate_events)
                }
            }
        }

        Layout::item_space(ui);

        // Auto-transition: Success + Create mode -> WaitingForOpponent + start poll.
        if let LobbyStatus::Success(game_id) = lobby.status {
            if lobby.mode == LobbyMode::Create && lobby.opponent_poll_rx.is_none() {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_poll_opponent_joined(lobby.cached_rpc_url.clone(), game_id, tx);
                lobby.opponent_poll_rx = Some(rx);
                lobby.status = LobbyStatus::WaitingForOpponent { game_id };
            } else if lobby.mode == LobbyMode::Join {
                // A confirmed join means this player is ready. Wait for the host GAME_START
                // before entering the match.
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_poll_game_start(game_id, tx);
                lobby.game_start_poll_rx = Some(rx);
                lobby.status = LobbyStatus::WaitingForHostStart { game_id };
            }
        } else if let LobbyStatus::EnterGame { game_id } = lobby.status {
            // Host's GAME_START signal arrived — safe to actually enter now.
            let wager_lamports = lobby.wager_lamports();
            start_solana_braid_game(
                &mut ctx.online_session,
                &ctx.network_config,
                ctx.network_state.as_ref().map(|r| &**r),
                ctx.p2p_state.as_deref_mut(),
                game_id,
                false,
                wager_lamports,
            );
            ctx.ai_config.mode = AiMode::Multiplayer;
            *ctx.core_mode = GameMode::OnlineMultiplayer;
            if let Some(ref mut sync) = ctx.solana_sync {
                sync.game_id = Some(game_id);
                sync.wager_amount = wager_lamports;
                sync.requires_delegation = wager_lamports > 0;
            }
            if let Some(ref mut comp) = ctx.competitive {
                comp.game_id = Some(game_id);
                comp.wager_lamports = wager_lamports;
                comp.active = true;
            }
            ctx.game_started_events
                .write(crate::game::events::GameStartedEvent { game_id });
            ctx.next_state.set(crate::core::GameState::InGame);
            ctx.menu_state.set(crate::core::MenuState::Main);
            // Prevent re-firing on the next frame while the state
            // transition above is still taking effect.
            lobby.status = LobbyStatus::Idle;
        }

        // Post-action status UI (uses ctx for firing events).
        let status_snap = lobby.status.clone();
        let wager_lamports = lobby.wager_lamports();

        match status_snap {
            LobbyStatus::Idle => {}

            LobbyStatus::Pending => {
                ui.spinner();
                ui.label(
                    egui::RichText::new("Submitting transaction...")
                        .color(egui::Color32::from_rgb(200, 200, 50)),
                );
            }

            LobbyStatus::WaitingForOpponent { game_id } => {
                // Refresh announcements every 25s, below the relay's 90s stale-lobby TTL.
                let should_heartbeat = lobby
                    .last_lobby_heartbeat
                    .map(|t| t.elapsed().as_secs() >= 25)
                    .unwrap_or(false); // announce just happened, first heartbeat at 25s
                if should_heartbeat {
                    if let Some(node_id) = lobby.cached_node_id.clone() {
                        lobby.last_lobby_heartbeat = Some(std::time::Instant::now());
                        crate::core::net_task::spawn_detached("screens", move || {
                            if let Err(e) = crate::multiplayer::vps_client::p2p_heartbeat(
                                game_id.to_string(),
                                &node_id,
                            ) {
                                warn!("[SOLANA_LOBBY] Heartbeat failed for game {}: {}", game_id, e);
                            }
                        });
                    }
                }

                ui.spinner();
                ui.label(
                    egui::RichText::new("Waiting for opponent to join...")
                        .color(egui::Color32::from_rgb(255, 200, 80)),
                );
                Layout::small_space(ui);

                // Game ID with copy button
                if game_id > 0 {
                    // Keep the on-chain game_id as a raw u64 PDA seed; formatting is display-only.
                    let base_minutes = lobby.time_control_base / 60;
                    let (_, category) = time_control_category(base_minutes);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Game ID: {} · {} {}+{}",
                                game_id, category, base_minutes, lobby.time_control_inc
                            ))
                            .strong(),
                        );
                        if ui
                            .small_button("Copy")
                            .on_hover_text("Copy game ID to clipboard")
                            .clicked()
                        {
                            ui.output_mut(|o| {
                                o.commands
                                    .push(egui::OutputCommand::CopyText(game_id.to_string()))
                            });
                        }
                    });
                }

                if let Some(ref warning) = lobby.announce_warning {
                    ui.label(
                        egui::RichText::new(format!("⚠ {}", warning))
                            .color(egui::Color32::from_rgb(230, 180, 60))
                            .size(11.0),
                    );
                }

                // Pot preview
                let wager = lobby.wager_sol;
                if wager > 0.0 {
                    let pot_line = match usd_per_sol.filter(|r| *r > 0.0) {
                        Some(rate) => format!(
                            "Your escrow: ${:.2}  |  Total pot: ${:.2}",
                            wager as f64 * rate,
                            wager as f64 * 2.0 * rate
                        ),
                        None => format!(
                            "Your escrow: {:.4} SOL  |  Total pot: {:.4} SOL",
                            wager,
                            wager * 2.0
                        ),
                    };
                    ui.label(
                        egui::RichText::new(pot_line)
                            .color(egui::Color32::GOLD)
                            .size(12.0),
                    );
                }

                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new(
                        "Share your Game ID with a friend or wait for someone from the lobby list.",
                    )
                    .size(12.0)
                    .color(egui::Color32::LIGHT_GRAY),
                );

                Layout::small_space(ui);
                if ui.small_button("Cancel").clicked() {
                    begin_solana_lobby_cancel(ctx, &mut lobby, game_id);
                }
            }

            LobbyStatus::OpponentJoined { game_id } => {
                // Until profile lookup completes, identify the opponent as Solana rather
                // than displaying an unverified name.
                ui.label(
                    egui::RichText::new("Opponent joined (Solana)!")
                        .color(egui::Color32::from_rgb(100, 255, 100))
                        .strong(),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new("Click 'Host Game' to start the match.")
                        .size(12.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );
                Layout::small_space(ui);
                if ui.button("Host Game").clicked() {
                    // Persist InProgress on the relay so joiners can poll it without missing a one-shot message.
                    if let Some(our_node_id) = lobby.cached_node_id.clone() {
                        let gid = game_id.to_string();
                        crate::core::net_task::spawn_detached("screens", move || {
                            let mut delivered = false;
                            for attempt in 1..=5u32 {
                                match crate::multiplayer::vps_client::p2p_accept_join(
                                    gid.clone(),
                                    &our_node_id,
                                ) {
                                    Ok(()) => {
                                        info!(
                                            "[SOLANA_LOBBY] Marked game {} in-progress (attempt {})",
                                            gid, attempt
                                        );
                                        delivered = true;
                                        break;
                                    }
                                    Err(e) => {
                                        warn!(
                                            "[SOLANA_LOBBY] p2p_accept_join failed (attempt {}/5) for {}: {}",
                                            attempt, gid, e
                                        );
                                        std::thread::sleep(std::time::Duration::from_millis(
                                            500 * attempt as u64,
                                        ));
                                    }
                                }
                            }
                            if !delivered {
                                error!(
                                    "[SOLANA_LOBBY] Could not mark {} in-progress after 5 attempts — joiner is likely stuck waiting",
                                    gid
                                );
                            }
                        });
                    } else {
                        warn!("[SOLANA_LOBBY] No node id available — can't mark game in-progress for joiner");
                    }

                    start_solana_braid_game(
                        &mut ctx.online_session,
                        &ctx.network_config,
                        ctx.network_state.as_ref().map(|r| &**r),
                        ctx.p2p_state.as_deref_mut(),
                        game_id,
                        true,
                        wager_lamports,
                    );
                    ctx.ai_config.mode = AiMode::Multiplayer;
                    *ctx.core_mode = GameMode::OnlineMultiplayer;
                    if let Some(ref mut sync) = ctx.solana_sync {
                        sync.game_id = Some(game_id);
                        sync.wager_amount = wager_lamports;
                        sync.requires_delegation = wager_lamports > 0;
                    }
                    if let Some(ref mut comp) = ctx.competitive {
                        comp.game_id = Some(game_id);
                        comp.wager_lamports = wager_lamports;
                        comp.active = true;
                    }
                    ctx.game_started_events
                        .write(crate::game::events::GameStartedEvent { game_id });
                    ctx.next_state.set(crate::core::GameState::InGame);
                    ctx.menu_state.set(crate::core::MenuState::Main);
                }
            }

            LobbyStatus::WaitingForHostStart { game_id } => {
                ui.spinner();
                ui.label(
                    egui::RichText::new("Joined! Waiting for the host to start the match...")
                        .color(egui::Color32::from_rgb(200, 200, 50)),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new("The host will begin the game when ready.")
                        .size(12.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );
                Layout::small_space(ui);
                if ui.small_button("Leave").clicked() {
                    begin_solana_lobby_cancel(ctx, &mut lobby, game_id);
                }
            }

            LobbyStatus::Cancelling { .. } => {
                ui.spinner();
                ui.label("Cancelling game and removing it from the lobby...");
                ui.colored_label(egui::Color32::GOLD, "Please wait for the refund result.");
            }

            LobbyStatus::Cancelled {
                refunded,
                ref message,
                ..
            } => {
                let color = if refunded {
                    egui::Color32::from_rgb(100, 255, 100)
                } else {
                    egui::Color32::LIGHT_GRAY
                };
                ui.colored_label(color, if refunded { "Game cancelled and wager refunded." } else { "Game removed from lobby." });
                ui.label(message);
                if ui.button("Back to lobby").clicked() {
                    lobby.status = LobbyStatus::Idle;
                    ctx.menu_state.set(crate::core::MenuState::ModeSelect);
                }
            }

            LobbyStatus::CancelFailed { game_id, ref error } => {
                ui.colored_label(egui::Color32::RED, "Cancellation did not complete.");
                ui.label(error);
                ui.label("Your game may still hold escrow. Retry cancellation before leaving.");
                if ui.button("Retry cancellation").clicked() {
                    begin_solana_lobby_cancel(ctx, &mut lobby, game_id);
                }
            }

            LobbyStatus::Fetched { .. } => {}

            // Auto-transition resolves this state before rendering; retained for exhaustiveness.
            LobbyStatus::Success(_) => {}

            // Resolved (back to Idle) in the auto-transition above before this
            // match ever renders it — kept only so the match stays exhaustive.
            LobbyStatus::EnterGame { .. } => {}

            LobbyStatus::Error(msg) => {
                ui.colored_label(egui::Color32::RED, format!(" {}", msg));
                if ui.small_button("Try Again").clicked() {
                    lobby.status = LobbyStatus::Idle;
                }
            }
        }
    });

    ctx.solana_lobby = Some(lobby);
}

/// List on-chain recoverable stakes and games awaiting ER undelegation.
/// Offer only cancel_game-compatible actions and refresh after cancellation.
#[cfg(feature = "solana")]
pub(super) fn render_stake_recovery(
    ui: &mut egui::Ui,
    ctx: &mut MainMenuUIContext,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
) {
    use crate::multiplayer::solana::wager_recovery::RecoveryAction;

    let busy = matches!(
        lobby.status,
        LobbyStatus::Pending
            | LobbyStatus::Cancelling { .. }
            | LobbyStatus::WaitingForOpponent { .. }
            | LobbyStatus::OpponentJoined { .. }
            | LobbyStatus::WaitingForHostStart { .. }
    );
    if lobby.recovery_rx.is_some() && lobby.recovery_scan.is_none() {
        ui.label(
            egui::RichText::new("Checking the chain for stakes you can recover...")
                .size(12.0)
                .color(egui::Color32::LIGHT_GRAY),
        );
        return;
    }
    let Some(scan) = lobby.recovery_scan.clone() else {
        return;
    };
    if scan.reclaimable.is_empty() && scan.in_play.is_empty() && scan.complete {
        return;
    }

    ui.separator();
    let mut reclaim: Option<(u64, u64)> = None;
    for wager in &scan.reclaimable {
        let sol = wager.wager_lamports as f64 / 1_000_000_000.0;
        let reason = match wager.action {
            RecoveryAction::CancelUnjoined => "no opponent joined",
            RecoveryAction::CancelBeforeFirstMove => "no move was played",
            RecoveryAction::CancelAbandoned => "game abandoned for over 24 hours",
            RecoveryAction::CompleteCancellation => "cancelled, refund not yet completed",
            RecoveryAction::UndelegationRequired => continue,
        };
        ui.horizontal(|ui| {
            ui.colored_label(
                egui::Color32::from_rgb(255, 220, 50),
                format!(
                    "Game {}: {sol:.4} SOL stake recoverable ({reason}).",
                    wager.game_id
                ),
            );
            if ui
                .add_enabled(!busy, egui::Button::new("Reclaim").small())
                .clicked()
            {
                reclaim = Some((wager.game_id, wager.wager_lamports));
            }
        });
    }
    for wager in &scan.in_play {
        let text = match (wager.action, wager.cancellable_in_secs) {
            (RecoveryAction::UndelegationRequired, _) => format!(
                "Game {}: stake held on the Ephemeral Rollup until the game is committed and settled.",
                wager.game_id
            ),
            (_, Some(secs)) => format!(
                "Game {}: stake held in an active game; it can be cancelled as abandoned in {}h if no one moves.",
                wager.game_id,
                (secs + 3599) / 3600
            ),
            _ => continue,
        };
        ui.label(
            egui::RichText::new(text)
                .size(12.0)
                .color(egui::Color32::LIGHT_GRAY),
        );
    }
    if !scan.complete {
        ui.colored_label(
            egui::Color32::from_rgb(255, 200, 100),
            "Could not fully check the chain for held stakes. Some may not be listed.",
        );
        if !scan.errors.is_empty() {
            warn!(
                "[WAGER-RECOVERY] Incomplete scan: {}",
                scan.errors.join("; ")
            );
            if let Some(rescan) = lobby.recovery_scan.as_mut() {
                rescan.errors.clear();
            }
        }
        if ui.small_button("Check again").clicked() {
            lobby.recovery_scan = None;
            lobby.rescan_recovery();
        }
    }
    ui.separator();

    if let Some((game_id, wager_lamports)) = reclaim {
        begin_solana_lobby_cancel_for(ctx, lobby, game_id, wager_lamports);
    }
}

#[cfg(feature = "solana")]
pub(super) fn begin_solana_lobby_cancel(
    ctx: &mut MainMenuUIContext,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    game_id: u64,
) {
    let wager_lamports = lobby.wager_lamports();
    begin_solana_lobby_cancel_for(ctx, lobby, game_id, wager_lamports);
}

#[cfg(feature = "solana")]
pub(super) fn begin_solana_lobby_cancel_for(
    ctx: &mut MainMenuUIContext,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    game_id: u64,
    wager_lamports: u64,
) {
    if matches!(lobby.status, LobbyStatus::Cancelling { .. }) {
        return;
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    spawn_cancel_lobby(
        lobby.cached_rpc_url.clone(),
        crate::solana::instructions::PROGRAM_ID
            .parse()
            .unwrap_or_default(),
        ctx.solana_state
            .as_ref()
            .and_then(|state| state.wallet_pubkey),
        game_id,
        wager_lamports,
        lobby.cached_node_id.clone(),
        tx,
    );
    lobby.cancel_rx = Some(rx);
    lobby.opponent_poll_rx = None;
    lobby.game_start_poll_rx = None;
    lobby.money_flow =
        crate::multiplayer::solana::money_flow::MoneyFlowState::signing("cancel_game");
    crate::multiplayer::network::vps::emit_client_event(
        crate::multiplayer::network::vps::ClientEvent::new("money_action_started")
            .game_id(game_id)
            .action("cancel_game")
            .status("signing"),
    );
    lobby.status = LobbyStatus::Cancelling { game_id };
}

#[cfg(feature = "solana")]
pub(super) fn render_session_health_hint(
    ui: &mut egui::Ui,
    wallet_connected: bool,
    has_global_session: bool,
    global_session_setup_in_progress: bool,
    global_session_unavailable_reason: Option<&str>,
    imported_session: Option<&crate::solana::session::SessionState>,
) {
    if let Some(session) = imported_session {
        match session.health {
            crate::solana::session::SessionHealth::Expired
            | crate::solana::session::SessionHealth::Invalid
            | crate::solana::session::SessionHealth::LoadFailed => {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 200, 50),
                    session
                        .recovery_message
                        .as_deref()
                        .unwrap_or("Reconnect your wallet from the launcher/web app."),
                );
                return;
            }
            crate::solana::session::SessionHealth::Connected => {
                ui.colored_label(
                    egui::Color32::from_rgb(80, 220, 120),
                    "Imported Solana session active",
                );
                return;
            }
            crate::solana::session::SessionHealth::Standalone => {}
        }
    }

    if !wallet_connected {
        ui.colored_label(
            egui::Color32::from_rgb(160, 130, 80),
            "Connect wallet to create or join on-chain games.",
        );
    } else if has_global_session {
        ui.colored_label(
            egui::Color32::from_rgb(80, 220, 120),
            "Quick-sign session active.",
        );
    } else if global_session_setup_in_progress {
        ui.colored_label(
            egui::Color32::from_rgb(200, 180, 120),
            "Verifying quick-sign session support...",
        );
    } else if let Some(reason) = global_session_unavailable_reason {
        ui.colored_label(
            egui::Color32::from_rgb(160, 160, 160),
            format!("Per-game signing fallback: {reason}"),
        );
    }
}

#[cfg(feature = "solana")]
pub(super) fn start_solana_braid_game(
    online_session: &mut crate::multiplayer::network::online_game_session::OnlineGameSession,
    network_config: &crate::multiplayer::types::NetworkConfig,
    network_state: Option<&crate::multiplayer::types::OnlineNetworkState>,
    p2p_state: Option<&mut crate::multiplayer::network::p2p::P2PConnectionState>,
    game_id: u64,
    is_host: bool,
    wager_lamports: u64,
) {
    if let Some(network_state) = network_state {
        crate::multiplayer::network::online_game_session::start_session(
            online_session,
            network_config.vps_base_url.clone(),
            game_id.to_string(),
            wager_lamports as f64 / 1_000_000_000.0,
            network_state,
        );
    }

    if let Some(p2p) = p2p_state {
        p2p.is_host = is_host;
        p2p.player_color = Some(if is_host {
            crate::rendering::pieces::PieceColor::White
        } else {
            crate::rendering::pieces::PieceColor::Black
        });
        p2p.status = P2PConnectionStatus::InGame;
    }
}

#[cfg(feature = "solana")]
pub(super) fn wallet_pubkey_from_cached(
    bytes: &Option<Vec<u8>>,
) -> Option<solana_sdk::pubkey::Pubkey> {
    let arr: [u8; 32] = bytes.as_deref()?.try_into().ok()?;
    Some(solana_sdk::pubkey::Pubkey::from(arr))
}
