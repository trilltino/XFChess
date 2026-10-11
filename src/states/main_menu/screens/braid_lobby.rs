use super::*;

pub(in super::super) fn render_braid_lobby_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ctx.learn_viewport.rect_px = None;

    ui.horizontal(|ui| {
        if ui.button(egui::RichText::new("Back").size(13.0)).clicked() {
            ctx.menu_state.set(crate::core::MenuState::Main);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(egui::RichText::new("⟳ Refresh").size(13.0))
                .clicked()
            {
                if let Some(ref mut vps) = ctx.p2p_vps_state {
                    vps.last_poll = None;
                }
            }
        });
    });

    ui.add_space(8.0);

    // Join by code
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Join by code:")
                .size(13.0)
                .color(egui::Color32::WHITE),
        );
        ui.add(
            egui::TextEdit::singleline(&mut ctx.competitive_menu.join_game_id)
                .hint_text("Enter code")
                .desired_width(160.0),
        );
        let can_join = !ctx.competitive_menu.join_game_id.trim().is_empty();
        if ui
            .add_enabled(
                can_join,
                egui::Button::new(
                    egui::RichText::new("Join")
                        .size(13.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(egui::Color32::from_rgb(40, 140, 80))
                .corner_radius(4.0),
            )
            .clicked()
        {
            // Uppercased so a hand-typed code always matches the host's
            // generated code regardless of the case the joiner typed it in.
            let game_id = ctx.competitive_menu.join_game_id.trim().to_uppercase();
            let local_node_id = ctx
                .network_state
                .as_ref()
                .and_then(|ns| {
                    ns.node_id
                        .as_ref()
                        .map(|id| bs58::encode(id.as_bytes()).into_string())
                })
                .unwrap_or_else(|| "unknown".to_string());
            let local_secret_key_bytes = ctx
                .network_state
                .as_ref()
                .and_then(|ns| ns.secret_key_bytes);
            let joiner_display_name = ctx.player_identity.display_name().to_string();
            let joiner_elo_str = ctx.player_identity.display_elo();
            let tx = ctx
                .p2p_vps_state
                .as_ref()
                .map(|vps| vps.response_tx.clone());
            if let Some(tx) = tx {
                let node_id_for_ack = local_node_id.clone();
                crate::core::net_task::spawn_detached("screens", move || {
                    match crate::multiplayer::network::vps::p2p_join_game(
                        game_id.clone(),
                        &local_node_id,
                    ) {
                        Ok(Some(host_id)) => {
                            let ack = format!(
                                "JOIN_ACK:{}|{}|{}",
                                node_id_for_ack, joiner_display_name, joiner_elo_str
                            );
                            let ack_result = match local_secret_key_bytes {
                                Some(sk) => crate::multiplayer::vps_client::p2p_send_message(
                                    game_id.clone(),
                                    &node_id_for_ack,
                                    &ack,
                                    &sk,
                                ),
                                None => Err("no signing key available yet".to_string()),
                            };
                            if let Err(e) = ack_result {
                                warn!("[LOBBY] JOIN_ACK send failed: {}", e);
                            }
                            let _ = tx.send(
                                crate::multiplayer::network::p2p_vps::VpsResponse::JoinResult {
                                    game_id,
                                    host_node_id: Some(host_id),
                                    stake_amount: 0.0,
                                },
                            );
                        }
                        Ok(None) => warn!("[LOBBY] Join by code rejected by VPS"),
                        Err(e) => error!("[LOBBY] Join by code error: {}", e),
                    }
                });
            }
            ctx.competitive_menu.join_game_id.clear();
        }
    });
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Filter:")
                .size(12.0)
                .color(egui::Color32::GRAY),
        );
        for (label, filter) in [
            ("All", crate::states::main_menu::LobbyFilter::All),
            ("Free", crate::states::main_menu::LobbyFilter::Free),
        ] {
            if ui
                .selectable_label(
                    ctx.competitive_menu.lobby_filter == filter,
                    egui::RichText::new(label).size(12.0),
                )
                .clicked()
            {
                ctx.competitive_menu.lobby_filter = filter;
            }
        }

        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("TC:")
                .size(12.0)
                .color(egui::Color32::GRAY),
        );
        for (label, min_opt, max_opt) in [
            ("Any", None, None),
            ("Bullet", None, Some(120u32)),
            ("Blitz", Some(121u32), Some(479u32)),
            ("Rapid+", Some(480u32), None),
        ] {
            let selected = ctx.competitive_menu.lobby_tc_min == min_opt
                && ctx.competitive_menu.lobby_tc_max == max_opt;
            if ui
                .selectable_label(selected, egui::RichText::new(label).size(12.0))
                .clicked()
            {
                ctx.competitive_menu.lobby_tc_min = min_opt;
                ctx.competitive_menu.lobby_tc_max = max_opt;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            use crate::multiplayer::social::LobbySort;
            egui::ComboBox::from_id_salt("lobby_sort")
                .selected_text(
                    egui::RichText::new(match ctx.competitive_menu.lobby_sort {
                        LobbySort::Newest => "Newest",
                        LobbySort::EloAsc => "ELO ↑",
                        LobbySort::EloDesc => "ELO ↓",
                        LobbySort::StakeAsc => "Newest",
                        LobbySort::StakeDesc => "Newest",
                        LobbySort::TimeAsc => "Time ↑",
                    })
                    .size(12.0),
                )
                .show_ui(ui, |ui| {
                    for (label, val) in [
                        ("Newest", LobbySort::Newest),
                        ("ELO ↑", LobbySort::EloAsc),
                        ("ELO ↓", LobbySort::EloDesc),
                        ("Time ↑", LobbySort::TimeAsc),
                    ] {
                        ui.selectable_value(&mut ctx.competitive_menu.lobby_sort, val, label);
                    }
                });
        });
    });
    ui.add_space(4.0);

    ui.heading(
        egui::RichText::new("Open Lobbies")
            .size(20.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(8.0);

    // Build + filter + sort the game list
    let all_games = ctx
        .p2p_vps_state
        .as_ref()
        .map(|v| v.cached_games.clone())
        .unwrap_or_default();
    let last_poll_age = ctx
        .p2p_vps_state
        .as_ref()
        .and_then(|v| v.last_poll.map(|t| t.elapsed().as_secs_f32()));
    let is_fetching = last_poll_age.map(|a| a < 1.5).unwrap_or(false) && all_games.is_empty();

    let filter = ctx.competitive_menu.lobby_filter;
    let tc_min = ctx.competitive_menu.lobby_tc_min;
    let tc_max = ctx.competitive_menu.lobby_tc_max;

    let mut games: Vec<_> = all_games
        .iter()
        .filter(|g| match filter {
            crate::states::main_menu::LobbyFilter::Free => g.stake_amount == 0.0,
            crate::states::main_menu::LobbyFilter::Wagered => g.stake_amount > 0.0,
            crate::states::main_menu::LobbyFilter::All => true,
        })
        .filter(|g| {
            tc_min.map(|m| g.base_time_seconds >= m).unwrap_or(true)
                && tc_max.map(|m| g.base_time_seconds <= m).unwrap_or(true)
        })
        .collect();

    use crate::multiplayer::social::LobbySort;
    match ctx.competitive_menu.lobby_sort {
        LobbySort::EloAsc => games.sort_by_key(|a| a.elo.unwrap_or(0)),
        LobbySort::EloDesc => games.sort_by(|a, b| b.elo.unwrap_or(0).cmp(&a.elo.unwrap_or(0))),
        LobbySort::StakeAsc => games.sort_by(|a, b| {
            a.stake_amount
                .partial_cmp(&b.stake_amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        LobbySort::StakeDesc => games.sort_by(|a, b| {
            b.stake_amount
                .partial_cmp(&a.stake_amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        LobbySort::TimeAsc => games.sort_by_key(|g| g.base_time_seconds),
        LobbySort::Newest => {}
    }

    if is_fetching {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            ui.spinner();
            ui.label(
                egui::RichText::new("Loading lobbies…")
                    .size(13.0)
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
        });
        return;
    }

    if games.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("No open lobbies right now.")
                    .size(14.0)
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Create one and wait for someone to join.")
                    .size(12.0)
                    .color(egui::Color32::from_rgba_unmultiplied(200, 200, 200, 120)),
            );
        });
        return;
    }

    let region_label = &ctx.backend_region.label;
    let region_latency = ctx.backend_region.latency_ms;
    #[cfg(feature = "solana")]
    let usd_per_sol = ctx
        .sol_usd_wager_rate
        .as_ref()
        .and_then(|r| r.current.as_ref())
        .map(|s| s.usd_per_sol)
        .filter(|r| *r > 0.0);
    #[cfg(not(feature = "solana"))]
    let usd_per_sol: Option<f64> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for game in &games {
            let name = game.username.as_deref().unwrap_or(game.display_name.as_str());
            let mins = game.base_time_seconds / 60;
            let inc  = game.increment_seconds;
            let time_str = if inc > 0 {
                format!("{}+{}", mins, inc)
            } else {
                format!("{} min", mins)
            };
            let stake_str = if game.stake_amount > 0.0 {
                match usd_per_sol {
                    Some(rate) => format!("${:.2}", game.stake_amount * rate),
                    None => format!("{:.3} SOL", game.stake_amount),
                }
            } else {
                "Free".to_string()
            };
            let stake_color = if game.stake_amount > 0.0 {
                egui::Color32::from_rgb(255, 190, 80)
            } else {
                egui::Color32::from_rgb(120, 200, 120)
            };

            // Capacity badge
            let capacity_str = format!("{}/{}", game.players_joined, game.capacity);
            let capacity_color = if game.players_joined >= game.capacity {
                egui::Color32::from_rgb(220, 80, 80)
            } else {
                egui::Color32::from_rgb(120, 200, 120)
            };

            // TTL countdown
            let ttl_str = if game.ttl_seconds > 0 {
                if game.ttl_seconds < 60 { format!("{}s", game.ttl_seconds) }
                else { format!("{}m", game.ttl_seconds / 60) }
            } else {
                String::new()
            };

            // Region with latency indicator
            let game_region = game.region.as_deref().unwrap_or("");
            let is_same_region = !game_region.is_empty()
                && (game_region == region_label.as_str() || game_region.contains(region_label.as_str()));
            let region_str = if game_region.is_empty() {
                String::new()
            } else if is_same_region {
                match region_latency {
                    Some(ms) if ms < 80  => format!("📡 {} {}ms", game_region, ms),
                    Some(ms) if ms < 200 => format!("📡 {} {}ms", game_region, ms),
                    Some(ms)             => format!("🌐 {} {}ms", game_region, ms),
                    None                 => format!("📡 {}", game_region),
                }
            } else {
                format!("🌐 {}", game_region)
            };
            let region_color = if is_same_region {
                match region_latency {
                    Some(ms) if ms < 80  => egui::Color32::from_rgb(80, 200, 120),
                    Some(ms) if ms < 200 => egui::Color32::from_rgb(240, 180, 60),
                    _                    => egui::Color32::from_rgb(120, 180, 220),
                }
            } else {
                egui::Color32::from_rgb(120, 180, 220)
            };

            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 50, 220))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Left: player info
                        ui.vertical(|ui| {
                            // Name row + private badge
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(name).size(14.0).color(egui::Color32::WHITE).strong());
                                if game.is_private {
                                    ui.label(egui::RichText::new("🔒").size(11.0).color(egui::Color32::GRAY));
                                }
                                // Capacity badge
                                ui.label(egui::RichText::new(&capacity_str).size(10.0).color(capacity_color));
                            });
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&time_str).size(11.5).color(egui::Color32::from_rgb(160, 190, 255)));
                                ui.label(egui::RichText::new("·").size(11.5).color(egui::Color32::GRAY));
                                ui.label(egui::RichText::new(&stake_str).size(11.5).color(stake_color));
                                if let Some(elo) = game.elo {
                                    ui.label(egui::RichText::new("·").size(11.5).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("{} ELO", elo)).size(11.5).color(egui::Color32::from_rgb(200, 160, 255)));
                                }
                                if !region_str.is_empty() {
                                    ui.label(egui::RichText::new("·").size(11.5).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(&region_str).size(10.5).color(region_color));
                                }
                                if !ttl_str.is_empty() {
                                    ui.label(egui::RichText::new("·").size(11.5).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("expires {}", ttl_str)).size(10.5).color(egui::Color32::from_rgb(180, 140, 80)));
                                }
                            });
                        });

                        // Right: join button
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let joinable = game.status == "Open" && game.players_joined < game.capacity;
                            if ui.add_enabled(joinable, egui::Button::new(
                                egui::RichText::new(if joinable { "Join" } else { "Busy" }).size(13.0).color(egui::Color32::WHITE).strong()
                            ).fill(if joinable { egui::Color32::from_rgb(40, 140, 80) } else { egui::Color32::from_rgb(60, 60, 60) }).corner_radius(4.0).min_size(egui::vec2(70.0, 32.0))).clicked() {
                                info!("[LOBBY] Joining: {}", game.game_id);
                                let game_id = game.game_id.clone();
                                let stake_amount = game.stake_amount;
                                let local_node_id = ctx.network_state.as_ref()
                                    .and_then(|ns| ns.node_id.as_ref().map(|id| bs58::encode(id.as_bytes()).into_string()))
                                    .unwrap_or_else(|| "unknown".to_string());
                                let local_secret_key_bytes = ctx.network_state.as_ref()
                                    .and_then(|ns| ns.secret_key_bytes);
                                let joiner_display_name = ctx.player_identity.display_name().to_string();
                                let joiner_elo_str = ctx.player_identity.display_elo();
                                let tx = ctx.p2p_vps_state.as_ref().map(|vps| vps.response_tx.clone());

                                if let Some(tx) = tx {
                                    crate::core::net_task::spawn_detached("screens", move || {
                                        match crate::multiplayer::network::vps::p2p_join_game(game_id.clone(), &local_node_id) {
                                            Ok(Some(host_id)) => {
                                                let ack = format!("JOIN_ACK:{}|{}|{}", local_node_id, joiner_display_name, joiner_elo_str);
                                                let ack_result = match local_secret_key_bytes {
                                                    Some(sk) => crate::multiplayer::vps_client::p2p_send_message(game_id.clone(), &local_node_id, &ack, &sk),
                                                    None => Err("no signing key available yet".to_string()),
                                                };
                                                if let Err(e) = ack_result {
                                                    warn!("[LOBBY] JOIN_ACK send failed: {}", e);
                                                } else {
                                                    info!("[LOBBY] Sent JOIN_ACK for {}", game_id);
                                                }
                                                let _ = tx.send(crate::multiplayer::network::p2p_vps::VpsResponse::JoinResult {
                                                    game_id,
                                                    host_node_id: Some(host_id),
                                                    stake_amount,
                                                });
                                            }
                                            Ok(None) => warn!("[LOBBY] Join rejected by VPS"),
                                            Err(e) => error!("[LOBBY] Join error: {}", e),
                                        }
                                    });
                                }
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }
    });
}
