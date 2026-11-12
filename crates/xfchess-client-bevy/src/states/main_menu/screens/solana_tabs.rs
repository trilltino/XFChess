use super::*;

#[cfg(feature = "solana")]
pub(super) fn render_create_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    compliance: &mut crate::ui::compliance_modal::ComplianceState,
    // Free games now go through spawn_create_game too, which reads
    // lobby.cached_node_id directly — this param is no longer needed here.
    _node_id: Option<&str>,
    usd_per_sol: Option<f64>,
    global_session_setup_in_progress: bool,
    global_session_unavailable_reason: Option<&str>,
    imported_session: Option<&crate::solana::session::SessionState>,
) {
    use crate::multiplayer::solana::lobby::EloMatchPref;

    ui.heading(
        egui::RichText::new("Create Game")
            .size(22.0)
            .color(egui::Color32::from_rgb(100, 200, 255))
            .strong(),
    );
    ui.add_space(12.0);

    let balance = lobby.cached_balance;
    let wallet_connected = lobby.cached_keypair_bytes.is_some();
    let max_wager = if wallet_connected {
        ((balance - 0.002) as f32).max(0.0)
    } else {
        0.0
    };

    ui.label(
        egui::RichText::new("XFChess | Competitive Chess Server")
            .color(egui::Color32::LIGHT_GRAY)
            .size(12.0)
            .italics(),
    );
    Layout::small_space(ui);

    // If wallet is not connected force wager to 0 (free game only).
    if !wallet_connected {
        lobby.wager_sol = 0.0;
    }

    // SOL is authoritative for the wager. Convert USD input when rates exist;
    // otherwise offer SOL input so price-feed failure does not block hosting.
    let live_rate = usd_per_sol.filter(|r| *r > 0.0);
    const PRESET_WAGERS_USD: [f32; 3] = [2.0, 5.0, 10.0];

    if wallet_connected {
        if let Some(rate) = live_rate {
            ui.label(egui::RichText::new("Wager amount (USD)").size(14.0));
            let max_wager_usd = (max_wager as f64 * rate) as f32;
            let wager_usd = (lobby.wager_sol as f64 * rate) as f32;

            let input_id = egui::Id::new("wager_custom_amount_usd");
            let editing = ui.memory(|m| m.has_focus(input_id));
            if !editing {
                // Resync from wager_sol only while the user is not editing.
                lobby.wager_amount_input = format!("{:.2}", wager_usd);
            }

            ui.horizontal(|ui| {
                for preset in PRESET_WAGERS_USD {
                    let affordable = preset <= max_wager_usd + 0.005;
                    let selected = affordable && (wager_usd - preset).abs() < 0.005;
                    let response = ui.add_enabled_ui(affordable, |ui| {
                        crate::ui::styles::StyledButton::chip(
                            ui,
                            &format!("${:.0}", preset),
                            selected,
                            egui::Vec2::new(48.0, 28.0),
                        )
                    });
                    if response.inner.clicked() {
                        lobby.wager_sol = (preset as f64 / rate) as f32;
                        lobby.wager_amount_input = format!("{:.2}", preset);
                    }
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Custom $").size(13.0));
                let edit_response = ui.add(
                    egui::TextEdit::singleline(&mut lobby.wager_amount_input)
                        .id(input_id)
                        .desired_width(60.0),
                );
                if edit_response.changed() {
                    if let Ok(value) = lobby.wager_amount_input.trim().parse::<f32>() {
                        let clamped = value.max(0.0).min(max_wager_usd);
                        lobby.wager_sol = (clamped as f64 / rate) as f32;
                    }
                }
            });
        } else {
            ui.label(egui::RichText::new("Wager amount (SOL)").size(14.0));
            let input_id = egui::Id::new("wager_custom_amount_sol");
            let editing = ui.memory(|m| m.has_focus(input_id));
            if !editing {
                lobby.wager_amount_input = format!("{:.3}", lobby.wager_sol);
            }
            let edit_response = ui.add(
                egui::TextEdit::singleline(&mut lobby.wager_amount_input)
                    .id(input_id)
                    .desired_width(80.0),
            );
            if edit_response.changed() {
                if let Ok(value) = lobby.wager_amount_input.trim().parse::<f32>() {
                    lobby.wager_sol = value.max(0.0).min(max_wager);
                }
            }
            ui.label(
                egui::RichText::new("(USD rate loading — showing SOL for now)")
                    .size(10.0)
                    .color(egui::Color32::from_rgb(160, 130, 80))
                    .italics(),
            );
        }
    } else {
        // Greyed-out disabled presets while no wallet is connected.
        ui.label(egui::RichText::new("Wager amount (USD)").size(14.0));
        ui.horizontal(|ui| {
            ui.add_enabled_ui(false, |ui| {
                for preset in PRESET_WAGERS_USD {
                    crate::ui::styles::StyledButton::chip(
                        ui,
                        &format!("${:.0}", preset),
                        false,
                        egui::Vec2::new(48.0, 28.0),
                    );
                }
            });
        });
        ui.label(
            egui::RichText::new("(Connect wallet to add a wager)")
                .size(11.0)
                .color(egui::Color32::from_rgb(160, 130, 80))
                .italics(),
        );
    }

    Layout::small_space(ui);

    ui.label(egui::RichText::new("Time Control").size(13.0));
    ui.horizontal_wrapped(|ui| {
        for (label, base, inc) in [
            ("Bullet 1+0", 60u32, 0u32),
            ("Blitz 3+2", 180, 2),
            ("Blitz 5+0", 300, 0),
            ("Rapid 10+0", 600, 0),
            ("Rapid 15+10", 900, 10),
            ("30 min", 1800, 0),
        ] {
            let selected = lobby.time_control_base == base && lobby.time_control_inc == inc;
            let response = crate::ui::styles::StyledButton::chip(
                ui,
                label,
                selected,
                egui::Vec2::new(0.0, 28.0),
            );
            if response.clicked() {
                lobby.time_control_base = base;
                lobby.time_control_inc = inc;
            }
            ui.add_space(3.0);
        }
    });

    Layout::small_space(ui);

    // ELO matching preference (only relevant for rated/wagered games)
    if lobby.match_type >= 1 || lobby.wager_sol > 0.0 {
        ui.label(egui::RichText::new("ELO Matching").size(13.0));
        ui.horizontal(|ui| {
            for pref in [
                EloMatchPref::Strict,
                EloMatchPref::Expanded,
                EloMatchPref::Any,
            ] {
                let selected = lobby.elo_pref == pref;
                let response = crate::ui::styles::StyledButton::chip(
                    ui,
                    pref.label(),
                    selected,
                    egui::Vec2::new(0.0, 28.0),
                );
                if response.clicked() {
                    lobby.elo_pref = pref;
                }
                ui.add_space(3.0);
            }
        });
        Layout::small_space(ui);
    }

    Layout::small_space(ui);

    let is_free_casual = lobby.wager_sol == 0.0 && lobby.match_type == 0;
    let is_free_rated = lobby.wager_sol == 0.0 && lobby.match_type == 1;
    // Free games still require a wallet signature for on-chain creation.
    let has_global_session = lobby.cached_global_session_keypair_bytes.is_some();
    // Wait for session setup before allowing a per-game signing fallback.
    let global_session_pending = global_session_setup_in_progress
        && !has_global_session
        && global_session_unavailable_reason.is_none();
    // Without a cached global session, free games may use per-game signing.
    let can_create = !matches!(lobby.status, LobbyStatus::Pending)
        && wallet_connected
        && !global_session_pending
        && (is_free_casual
            || is_free_rated
            || (lobby.wager_sol > 0.0 && (lobby.wager_sol as f64) <= balance - 0.002));

    let is_devnet = lobby.cached_rpc_url.contains("devnet");

    if global_session_pending {
        ui.colored_label(
            egui::Color32::from_rgb(200, 180, 120),
            "⏳ Setting up one-time session signing…",
        );
        Layout::small_space(ui);
    } else if wallet_connected && !has_global_session {
        // Show the platform fee before opening the wallet signature prompt.
        ui.colored_label(
            egui::Color32::from_rgb(160, 160, 160),
            if let Some(reason) = global_session_unavailable_reason {
                format!("Per-game signing this session ({reason}) — you'll also approve a small platform fee.")
            } else {
                "Per-game signing — you'll also approve a small platform fee for this game.".to_string()
            },
        );
        Layout::small_space(ui);
    }
    render_session_health_hint(
        ui,
        wallet_connected,
        has_global_session,
        global_session_setup_in_progress,
        global_session_unavailable_reason,
        imported_session,
    );
    Layout::small_space(ui);

    let create_btn_text = if is_free_casual {
        "Host Free Game"
    } else if is_free_rated {
        "Host Free Rated Game"
    } else {
        "Create Wagered Game"
    };

    let create_clicked = ui
        .add_sized(
            [ui.available_width(), 40.0],
            egui::Button::new(egui::RichText::new(create_btn_text).size(16.0).strong()).fill(
                if can_create {
                    egui::Color32::from_rgb(40, 100, 40)
                } else {
                    egui::Color32::from_rgb(40, 40, 40)
                },
            ),
        )
        .clicked();
    if create_clicked && !can_create {
        let reason = if !wallet_connected {
            "wallet not connected"
        } else if global_session_pending {
            "global session pending"
        } else if wallet_connected && balance < 0.003 {
            "insufficient balance"
        } else {
            "create unavailable"
        };
        crate::multiplayer::network::vps::emit_client_event(
            crate::multiplayer::network::vps::ClientEvent::new("solana_create_game_blocked")
                .session_kind(if has_global_session {
                    "global"
                } else {
                    "per_game"
                })
                .reason(reason),
        );
    }
    if create_clicked && can_create {
        // Require CARF compliance only for mainnet wagers; free games use wager_amount=0.
        let needs_compliance = !is_free_casual
            && !is_devnet
            && compliance.status != crate::ui::compliance_modal::SubmissionStatus::Success;
        if needs_compliance {
            compliance.show = true;
            if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                compliance.pubkey = Some(wallet_pubkey.to_string());
            }
        } else if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
            let (tx, rx) = tokio::sync::oneshot::channel();
            lobby.last_attempt_used_global_session =
                lobby.cached_global_session_keypair_bytes.is_some();
            spawn_create_game(
                lobby.cached_rpc_url.clone(),
                wallet_pubkey,
                lobby.wager_lamports(),
                lobby.match_type,
                lobby.time_control_base,
                lobby.time_control_inc,
                lobby.cached_global_session_keypair_bytes.clone(),
                tx,
            );
            lobby.tx_rx = Some(rx);
            lobby.status = LobbyStatus::Pending;
            lobby.announce_warning = None;
            info!(
                "[SOLANA_LOBBY] Creating game ({} SOL wager)",
                lobby.wager_sol
            );
        }
    }

    if !can_create && wallet_connected && balance < 0.003 {
        ui.colored_label(
            egui::Color32::RED,
            "Insufficient balance (need = 0.003 SOL)",
        );
    }
}

#[cfg(feature = "solana")]
pub(super) fn render_join_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    node_id: Option<&str>,
    secret_key_bytes: Option<[u8; 32]>,
    usd_per_sol: Option<f64>,
    global_session_setup_in_progress: bool,
    global_session_unavailable_reason: Option<&str>,
    imported_session: Option<&crate::solana::session::SessionState>,
) {
    ui.label(egui::RichText::new("Enter Game ID:").size(14.0));
    ui.text_edit_singleline(&mut lobby.game_id_input);

    Layout::small_space(ui);

    let game_id_valid = lobby.game_id_input.trim().parse::<u64>().is_ok();
    let looking_up = matches!(lobby.status, LobbyStatus::Pending);
    let is_idle = matches!(lobby.status, LobbyStatus::Idle);

    // Auto-lookup only from Idle; rerunning after join success would overwrite
    // the transaction result with a lookup of an already-active game.
    if game_id_valid && is_idle && !lobby.game_id_input.is_empty() {
        if let Ok(game_id) = lobby.game_id_input.trim().parse::<u64>() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            spawn_lookup_game(lobby.cached_rpc_url.clone(), game_id, tx);
            lobby.lookup_rx = Some(rx);
            lobby.status = LobbyStatus::Pending;
            info!("[SOLANA_LOBBY] Automatic lookup for game {}", game_id);
        }
    }

    if ui
        .add_sized(
            [ui.available_width(), 30.0],
            egui::Button::new("Manual Look Up"),
        )
        .clicked()
        && game_id_valid
        && !looking_up
    {
        if let Ok(game_id) = lobby.game_id_input.trim().parse::<u64>() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            spawn_lookup_game(lobby.cached_rpc_url.clone(), game_id, tx);
            lobby.lookup_rx = Some(rx);
            lobby.status = LobbyStatus::Pending;
            info!("[SOLANA_LOBBY] Manual lookup for game {}", game_id);
        }
    }

    // Show fetched wager info + confirm join button
    if let LobbyStatus::Fetched { wager_sol, game_id } = lobby.status {
        ui.separator();
        Layout::small_space(ui);
        let wager_line = match usd_per_sol.filter(|r| *r > 0.0) {
            Some(rate) => format!(
                "Game #{} requires ${:.2} (≈{:.4} SOL) wager",
                game_id,
                wager_sol * rate,
                wager_sol
            ),
            None => format!("Game #{} requires {:.4} SOL wager", game_id, wager_sol),
        };
        ui.label(egui::RichText::new(wager_line).color(egui::Color32::GOLD));
        let balance_line = match usd_per_sol.filter(|r| *r > 0.0) {
            Some(rate) => format!(
                "Your balance: ${:.2} (≈{:.4} SOL)",
                lobby.cached_balance * rate,
                lobby.cached_balance
            ),
            None => format!("Your balance: {:.4} SOL", lobby.cached_balance),
        };
        ui.label(egui::RichText::new(balance_line).size(12.0));

        let sufficient = lobby.cached_balance >= wager_sol + 0.002;
        let has_global_session = lobby.cached_global_session_keypair_bytes.is_some();
        let global_session_pending = global_session_setup_in_progress
            && !has_global_session
            && global_session_unavailable_reason.is_none();
        let can_join = !matches!(lobby.status, LobbyStatus::Pending)
            && lobby.cached_keypair_bytes.is_some()
            && sufficient
            && !global_session_pending;

        if !sufficient {
            ui.colored_label(egui::Color32::RED, "Insufficient balance to join");
        }
        if global_session_pending {
            ui.colored_label(
                egui::Color32::from_rgb(200, 180, 120),
                "⏳ Setting up one-time session signing…",
            );
        } else if !has_global_session {
            ui.colored_label(
                egui::Color32::from_rgb(160, 160, 160),
                "Per-game signing this session — you'll approve the join transaction directly.",
            );
        }

        render_session_health_hint(
            ui,
            lobby.cached_keypair_bytes.is_some(),
            has_global_session,
            global_session_setup_in_progress,
            global_session_unavailable_reason,
            imported_session,
        );

        let join_clicked = ui
            .add_enabled(can_join, egui::Button::new("Confirm Join"))
            .clicked();
        if join_clicked && !can_join {
            let reason = if lobby.cached_keypair_bytes.is_none() {
                "wallet not connected"
            } else if global_session_pending {
                "global session pending"
            } else if !sufficient {
                "insufficient balance"
            } else {
                "join unavailable"
            };
            crate::multiplayer::network::vps::emit_client_event(
                crate::multiplayer::network::vps::ClientEvent::new("solana_join_game_blocked")
                    .game_id(game_id)
                    .session_kind(if has_global_session {
                        "global"
                    } else {
                        "per_game"
                    })
                    .reason(reason),
            );
        }
        if join_clicked && can_join {
            if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                if let Some(node_id) = node_id {
                    match crate::multiplayer::vps_client::p2p_join_game(
                        game_id.to_string(),
                        node_id,
                    ) {
                        Ok(Some(host_id)) => {
                            info!(
                                "[SOLANA_LOBBY] Relay join accepted for game {} (host {})",
                                game_id, host_id
                            );
                            // Send JOIN_ACK with the node ID so the host can establish move transport.
                            let display = lobby
                                .cached_display_name
                                .clone()
                                .unwrap_or_else(|| "Anonymous".to_string());
                            let elo = lobby.cached_elo;
                            let ack = format!("JOIN_ACK:{}|{}|{}", node_id, display, elo);
                            let ack_result = match secret_key_bytes {
                                Some(sk) => crate::multiplayer::vps_client::p2p_send_message(
                                    game_id.to_string(),
                                    node_id,
                                    &ack,
                                    &sk,
                                ),
                                None => Err("no signing key available yet".to_string()),
                            };
                            if let Err(e) = ack_result {
                                warn!("[SOLANA_LOBBY] JOIN_ACK send failed for {}: {}", game_id, e);
                            }
                        }
                        Ok(None) => warn!(
                            "[SOLANA_LOBBY] Relay join accepted without host node for game {}",
                            game_id
                        ),
                        Err(e) => warn!("[SOLANA_LOBBY] Relay join failed for {}: {}", game_id, e),
                    }
                }
                let (tx, rx) = tokio::sync::oneshot::channel();
                lobby.last_attempt_used_global_session =
                    lobby.cached_global_session_keypair_bytes.is_some();
                spawn_join_game(
                    lobby.cached_rpc_url.clone(),
                    wallet_pubkey,
                    game_id,
                    lobby.cached_global_session_keypair_bytes.clone(),
                    tx,
                );
                // Copy fetched wager into create flow so poll_lobby_tasks can persist it
                lobby.wager_sol = wager_sol as f32;
                lobby.tx_rx = Some(rx);
                lobby.status = LobbyStatus::Pending;
                info!(
                    "[SOLANA_LOBBY] Joining game {} (wager {:.4} SOL)",
                    game_id, wager_sol
                );
            }
        }
    }
}

#[cfg(feature = "solana")]
pub(super) fn render_solana_browse_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    mut vps_state: Option<&mut crate::multiplayer::network::p2p_vps::P2PVpsState>,
    usd_per_sol: Option<f64>,
) {
    use crate::multiplayer::solana::lobby::{spawn_join_game, LobbyStatus};

    let is_loading = vps_state
        .as_ref()
        .is_some_and(|vps| vps.is_game_list_loading());
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Open Games")
                .size(14.0)
                .strong()
                .color(egui::Color32::GOLD),
        );
        if is_loading {
            ui.spinner();
        } else if ui.small_button("Refresh").clicked() {
            if let Some(vps) = vps_state.as_mut() {
                vps.last_poll = None;
            }
        }
    });
    ui.add_space(6.0);

    if is_loading && lobby.browse_games.is_empty() {
        ui.label(
            egui::RichText::new("Loading…")
                .size(12.0)
                .color(egui::Color32::GRAY)
                .italics(),
        );
        return;
    }

    // Include zero-stake and wagered on-chain games in Solana browse results.
    let games = lobby.browse_games.clone();
    if games.is_empty() {
        ui.label(
            egui::RichText::new("No open games right now.")
                .size(12.0)
                .color(egui::Color32::GRAY),
        );
        return;
    }

    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
        for game in &games {
            let name = game.username.as_deref().unwrap_or(game.display_name.as_str());
            let mins = game.base_time_seconds / 60;
            let inc  = game.increment_seconds;
            let time_str = if inc > 0 { format!("{}+{}", mins, inc) } else { format!("{} min", mins) };
            let stake_str = if game.stake_amount > 0.0 {
                match usd_per_sol.filter(|r| *r > 0.0) {
                    Some(rate) => format!("${:.2}", game.stake_amount * rate),
                    None => format!("{:.3} SOL", game.stake_amount),
                }
            } else {
                "Free".to_string()
            };
            let stake_color = if game.stake_amount > 0.0 {
                egui::Color32::GOLD
            } else {
                egui::Color32::from_rgb(120, 200, 120)
            };

            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(40, 35, 20, 220))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 80, 20)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(name).size(13.0).color(egui::Color32::WHITE).strong());
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&time_str).size(11.0).color(egui::Color32::from_rgb(160, 190, 255)));
                                ui.label(egui::RichText::new("·").size(11.0).color(egui::Color32::GRAY));
                                ui.label(egui::RichText::new(&stake_str).size(11.0).color(stake_color));
                                if let Some(elo) = game.elo {
                                    ui.label(egui::RichText::new("·").size(11.0).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("{} ELO", elo)).size(11.0).color(egui::Color32::from_rgb(200, 160, 255)));
                                }
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let joinable = game.status == "Open";
                            let full = !joinable || game.players_joined >= game.capacity;
                            let can_join = !matches!(lobby.status, LobbyStatus::Pending)
                                && lobby.cached_keypair_bytes.is_some()
                                && lobby.cached_balance >= game.stake_amount + 0.002
                                && joinable
                                && game.players_joined < game.capacity;
                            if ui.add_enabled(can_join, egui::Button::new(
                                egui::RichText::new(if full { "Full" } else { "Join" }).size(12.0).strong()
                            ).fill(if can_join { egui::Color32::from_rgb(140, 80, 0) } else { egui::Color32::from_rgb(60, 60, 60) }).corner_radius(4.0).min_size(egui::vec2(60.0, 28.0))).clicked() {
                                if let Some(pk) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                                    // Parse on-chain game IDs strictly as u64 PDA seeds; never use a hash fallback.
                                    if let Ok(game_id) = game.game_id.parse::<u64>() {
                                        if let Some(node_id) = lobby.cached_node_id.clone() {
                                            match crate::multiplayer::vps_client::p2p_join_game(game.game_id.clone(), &node_id) {
                                                Ok(Some(host_id)) => {
                                                    info!("[SOLANA_BROWSE] Relay join accepted for game {} (host {})", game.game_id, host_id);
                                                    // See render_join_tab's Confirm Join handler for why
                                                    // this JOIN_ACK is required for the game to connect.
                                                    let display = lobby
                                                        .cached_display_name
                                                        .clone()
                                                        .unwrap_or_else(|| "Anonymous".to_string());
                                                    let elo = lobby.cached_elo;
                                                    let ack = format!("JOIN_ACK:{}|{}|{}", node_id, display, elo);
                                                    let ack_result = match lobby.cached_secret_key_bytes {
                                                        Some(sk) => crate::multiplayer::vps_client::p2p_send_message(game.game_id.clone(), &node_id, &ack, &sk),
                                                        None => Err("no signing key available yet".to_string()),
                                                    };
                                                    if let Err(e) = ack_result {
                                                        warn!("[SOLANA_BROWSE] JOIN_ACK send failed for {}: {}", game.game_id, e);
                                                    }
                                                }
                                                Ok(None) => warn!("[SOLANA_BROWSE] Relay join accepted without host node for game {}", game.game_id),
                                                Err(e) => warn!("[SOLANA_BROWSE] Relay join failed for {}: {}", game.game_id, e),
                                            }
                                        }
                                        let (tx, rx) = tokio::sync::oneshot::channel();
                                        lobby.last_attempt_used_global_session =
                                            lobby.cached_global_session_keypair_bytes.is_some();
                                        spawn_join_game(
                                            lobby.cached_rpc_url.clone(),
                                            pk,
                                            game_id,
                                            lobby.cached_global_session_keypair_bytes.clone(),
                                            tx,
                                        );
                                        lobby.wager_sol = game.stake_amount as f32;
                                        lobby.tx_rx = Some(rx);
                                        lobby.status = LobbyStatus::Pending;
                                        lobby.game_id_input = game_id.to_string();
                                        lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Join;
                                    }
                                }
                            }
                        });
                    });
                });
            ui.add_space(4.0);
        }
    });
}

#[cfg(feature = "solana")]
pub(super) fn render_solana_tournament_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    spectate_writer: &mut Option<
        crate::multiplayer::traits::MessageWriter<
            crate::multiplayer::spectator::SpectateViaLinkEvent,
        >,
    >,
) {
    let is_loading = lobby.tournament_rx.is_some();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Tournament Games")
                .size(14.0)
                .strong()
                .color(egui::Color32::GOLD),
        );
        if is_loading {
            ui.spinner();
        } else if ui.small_button("Refresh").clicked() {
            lobby.tournament_last_fetch = None;
        }
    });
    ui.add_space(6.0);

    if is_loading && lobby.tournament_games.is_empty() {
        ui.label(
            egui::RichText::new("Loading…")
                .size(12.0)
                .color(egui::Color32::GRAY)
                .italics(),
        );
        return;
    }

    if lobby.tournament_games.is_empty() {
        ui.label(
            egui::RichText::new("No on-chain tournament games right now.")
                .size(12.0)
                .color(egui::Color32::GRAY),
        );
        return;
    }

    let games = lobby.tournament_games.clone();
    egui::ScrollArea::vertical()
        .max_height(280.0)
        .show(ui, |ui| {
            for game in &games {
                let status_color = match game.status.as_str() {
                    "Active" => egui::Color32::from_rgb(120, 220, 120),
                    "Completed" => egui::Color32::GRAY,
                    _ => egui::Color32::from_rgb(220, 200, 120),
                };
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(25, 30, 45, 220))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 80, 130)))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&game.tournament_name)
                                        .size(13.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                );
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("Round {}", game.round + 1))
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(160, 190, 255)),
                                    );
                                    ui.label(
                                        egui::RichText::new("·")
                                            .size(11.0)
                                            .color(egui::Color32::GRAY),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} vs {}",
                                            game.white_label(),
                                            game.black_label()
                                        ))
                                        .size(11.0)
                                        .color(egui::Color32::LIGHT_GRAY),
                                    );
                                    ui.label(
                                        egui::RichText::new("·")
                                            .size(11.0)
                                            .color(egui::Color32::GRAY),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("Game {}", game.game_id))
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(200, 160, 255)),
                                    );
                                });
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let is_active = game.status == "Active";
                            if ui
                                .add_enabled(
                                    is_active,
                                    egui::Button::new(
                                        egui::RichText::new("Watch")
                                            .size(12.0)
                                            .color(egui::Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(if is_active {
                                        egui::Color32::from_rgb(60, 100, 160)
                                    } else {
                                        egui::Color32::from_rgb(60, 60, 60)
                                    })
                                    .corner_radius(4.0)
                                    .min_size(egui::vec2(60.0, 28.0)),
                                )
                                .clicked()
                            {
                                if let Some(ref mut w) = spectate_writer {
                                    w.write(crate::multiplayer::spectator::SpectateViaLinkEvent {
                                        game_id: game.game_id.to_string(),
                                        details: Some(
                                            crate::multiplayer::spectator::SpectatorMatchDetails {
                                                tournament_name: Some(game.tournament_name.clone()),
                                                round: Some(game.round),
                                                white: Some(game.white_label()),
                                                black: Some(game.black_label()),
                                            },
                                        ),
                                        tournament_id: Some(game.tournament_id),

                                        playlist: Vec::new(),
                                    });
                                }
                            }
                            ui.label(
                                egui::RichText::new(&game.status)
                                    .size(11.0)
                                    .color(status_color)
                                    .strong(),
                            );
                        });
                        });
                    });
                ui.add_space(4.0);
            }
        });
}
