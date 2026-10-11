use super::*;

pub(in super::super) fn render_spectator_popup(
    ctx: &egui::Context,
    competitive: &mut CompetitiveMenuState,
    cached_games: &[crate::multiplayer::network::p2p_vps::VpsGameListing],
    spectate_writer: &mut Option<
        crate::multiplayer::traits::MessageWriter<
            crate::multiplayer::spectator::SpectateViaLinkEvent,
        >,
    >,
    sol_usd_rate: Option<f64>,
) {
    egui::Window::new("Spectator Mode")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::Vec2::new(520.0, 380.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .title_bar(false)
        .order(egui::Order::Foreground)
        .frame(crate::ui::styles::StyledPanel::popup())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Spectator Mode")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        competitive.set_active_modal(None);
                    }
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // Spectate only games with both seats occupied; relay status alone does
            // not establish this for tournament and casual start flows.
            let live_games: Vec<_> = cached_games
                .iter()
                .filter(|g| g.status == "InProgress" && g.players_joined >= g.capacity)
                .collect();

            if live_games.is_empty() {
                ui.label(
                    egui::RichText::new("No games available to spectate at the moment.")
                        .size(14.0)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                );
                ui.label(
                    egui::RichText::new("Check back later for live games.")
                        .size(12.0)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 120))
                        .italics(),
                );
            } else {
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for game in live_games {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    let host = game.username.as_deref().unwrap_or(&game.display_name);
                                    ui.label(egui::RichText::new(host).size(13.0).color(egui::Color32::WHITE).strong());
                                    let type_badge = match game.game_type.as_str() {
                                        "solana_wager" => " Wager",
                                        // Tournament relay announcements use tournament_match.
                                        "tournament" | "tournament_match" => " Tournament",
                                        _ => " Free",
                                    };
                                    let stake_str = if game.stake_amount > 0.0 {
                                        match sol_usd_rate.map(|rate| game.stake_amount * rate) {
                                            Some(usd) => format!("{} — ${:.2}", type_badge, usd),
                                            None => format!("{} — {:.3} SOL", type_badge, game.stake_amount),
                                        }
                                    } else {
                                        type_badge.to_string()
                                    };
                                    ui.label(egui::RichText::new(&stake_str).size(11.0).color(egui::Color32::LIGHT_GRAY));
                                    let tc_label = if game.base_time_seconds > 0 {
                                        let m = game.base_time_seconds / 60;
                                        let s = game.base_time_seconds % 60;
                                        if s == 0 { format!("? {}+{}", m, game.increment_seconds) }
                                        else { format!("? {}s+{}", game.base_time_seconds, game.increment_seconds) }
                                    } else { "? 8".to_string() };
                                    ui.label(egui::RichText::new(tc_label).size(10.0).color(egui::Color32::GRAY));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let game_id = game.game_id.clone();
                                    if ui.add_sized([60.0, 28.0], egui::Button::new(
                                        egui::RichText::new("Watch").size(11.0).color(egui::Color32::WHITE).strong()
                                    ).fill(egui::Color32::from_rgb(60, 100, 160)).corner_radius(4.0)).clicked() {
                                        if let Some(ref mut w) = spectate_writer {
                                            w.write(crate::multiplayer::spectator::SpectateViaLinkEvent {
                                                game_id,
                                                details: Some(crate::multiplayer::spectator::SpectatorMatchDetails {
                                                    white: Some(game.username.clone().unwrap_or_else(|| game.display_name.clone())),
                                                    ..Default::default()
                                                }),
                                                tournament_id: None,

                                                playlist: Vec::new(),
                                            });
                                            competitive.set_active_modal(None);
                                        }
                                    }
                                });
                            });
                        });
                        ui.add_space(4.0);
                    }
                });
            }

            ui.add_space(12.0);

            if ui.add_sized(
                [ui.available_width(), 36.0],
                egui::Button::new(egui::RichText::new("Close").size(14.0).color(egui::Color32::WHITE).strong())
                    .fill(crate::ui::styles::UiColors::ACCENT)
                    .corner_radius(4.0),
            ).clicked() {
                competitive.set_active_modal(None);
            }
        });
}

pub(in super::super) fn render_loading_screen_website(
    ctx: &egui::Context,
    ctx_menu: &mut MainMenuUIContext,
) {
    let screen_rect = ctx.input(|i| i.content_rect());

    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: egui::Color32::from_rgb(0, 0, 0),
            ..egui::Frame::NONE
        })
        .show(
            &mut crate::ui::egui_ext::viewport_ui(ctx, "screens_root"),
            |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(screen_rect.height() * 0.4);

                    if ctx_menu.loading_progress.failed {
                        ui.heading(
                            egui::RichText::new("Asset Loading Failed")
                                .size(24.0)
                                .color(egui::Color32::from_rgb(220, 50, 50)),
                        );

                        if let Some(ref error_msg) = ctx_menu.loading_progress.error_message {
                            ui.label(
                                egui::RichText::new(error_msg)
                                    .size(14.0)
                                    .color(egui::Color32::from_rgb(220, 150, 150)),
                            );
                        }

                        if ui.button("Continue Anyway").clicked() {
                            ctx_menu.loading_progress.complete = true;
                            ctx_menu.game_assets.loaded = true;
                        }
                    } else {
                        ui.heading(
                            egui::RichText::new("Loading...")
                                .size(24.0)
                                .color(egui::Color32::WHITE),
                        );

                        let progress_bar =
                            egui::ProgressBar::new(ctx_menu.loading_progress.progress)
                                .desired_width(300.0)
                                .show_percentage()
                                .animate(true);

                        ui.add(progress_bar);
                    }
                });
            },
        );
}

pub(in super::super) fn render_lobby_selection_popup(
    mut contexts: bevy_egui::EguiContexts,
    mut menu_state: ResMut<NextState<crate::core::MenuState>>,
    #[cfg(feature = "solana")] solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
) {
    let Some(ctx) = contexts.ctx_mut().ok() else {
        return;
    };

    egui::Window::new("Create Multiplayer Lobby")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(320.0)
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(15, 15, 20))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(230, 57, 70)))
                .corner_radius(12.0)
                .inner_margin(24.0),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Choose Lobby Type")
                    .size(20.0)
                    .color(egui::Color32::from_rgb(230, 57, 70))
                    .strong());

                ui.add_space(12.0);
                ui.label(egui::RichText::new("Select how you want to play against others.")
                    .color(egui::Color32::GRAY)
                    .size(13.0));

                ui.add_space(32.0);

                // Local Button
                let reg_btn = ui.add_sized(
                    [240.0, 48.0],
                    egui::Button::new(
                        egui::RichText::new("Local").size(16.0).strong()
                    )
                    .fill(egui::Color32::from_rgb(40, 40, 50))
                ).on_hover_text("Standard match without on-chain wagering");

                if reg_btn.clicked() {
                    menu_state.set(crate::core::MenuState::BraidLobby);
                }

                ui.add_space(16.0);

                // Solana Wager P2P Button (only available with solana feature)
                #[cfg(feature = "solana")]
                {
                    let solana_available = solana_state.as_ref().map(|s| s.wallet_pubkey.is_some()).unwrap_or(false);
                    let sol_btn = ui.add_sized(
                        [240.0, 48.0],
                        egui::Button::new(egui::RichText::new("Solana Wager P2P").size(16.0).strong())
                            .fill(if solana_available { egui::Color32::from_rgb(230, 57, 70) } else { egui::Color32::from_rgb(80, 80, 80) })
                    ).on_hover_text(if solana_available { "Play for SOL on-chain via Solana Devnet" } else { "Solana not available - feature disabled" });

                    if sol_btn.clicked() {
                        if let Some(state) = solana_state.as_ref() {
                            if state.wallet_pubkey.is_none() {
                                info!("[MENU] Solana wager blocked — wallet not connected. Opening wallet popup.");
                                tauri_signer::open_wallet_browser();
                            } else if state.profile_status != crate::multiplayer::solana::integration::state::ProfileStatus::HasProfileWithUsername {
                                info!("[MENU] Profile missing or incomplete. Opening Tauri profile step.");
                                tauri_signer::open_profile_step();
                            } else {
                                menu_state.set(crate::core::MenuState::SolanaLobby);
                            }
                        }
                    }
                }

                #[cfg(not(feature = "solana"))]
                {
                    ui.add_enabled_ui(false, |ui| {
                        ui.add_sized(
                            [240.0, 48.0],
                            egui::Button::new(egui::RichText::new("Solana Wager P2P (Disabled)").size(16.0).strong())
                                .fill(egui::Color32::from_rgb(80, 80, 80))
                        ).on_hover_text("Solana feature not enabled in this build");
                    });
                }

                ui.add_space(32.0);

                if ui.button(egui::RichText::new("Cancel").color(egui::Color32::GRAY)).clicked() {
                    menu_state.set(crate::core::MenuState::Main);
                }

                ui.add_space(8.0);
            });
        });
}
