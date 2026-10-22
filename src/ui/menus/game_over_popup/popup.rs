use super::*;

pub(super) fn paint_black_backdrop(ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::default().fill(egui::Color32::BLACK))
        .show(
            &mut crate::ui::egui_ext::viewport_ui(ctx, "game_over_popup_root"),
            |_ui| {},
        );
}

pub fn game_over_popup_system(
    mut contexts: EguiContexts,
    game_over: Res<GameOverState>,
    payout_info: Option<Res<GameOverPayoutInfo>>,
    cached_pgn: Res<CachedGamePgn>,
    mut next_state: ResMut<NextState<GameState>>,
    mut game_mode: ResMut<crate::core::GameMode>,
    mut commands: Commands,
    mut popup_queue: ResMut<crate::ui::menus::popup::GamePopupQueue>,
    tokio_runtime: Option<Res<crate::multiplayer::TokioRuntime>>,
    player_identity: Res<crate::states::main_menu::PlayerIdentity>,
    #[cfg(feature = "solana")] sol_usd_rate: Option<
        Res<crate::multiplayer::solana::wager_rate::SolUsdRate>,
    >,
) {
    #[cfg(feature = "solana")]
    let usd_per_sol = sol_usd_rate
        .as_ref()
        .and_then(|r| r.snapshot())
        .map(|s| s.usd_per_sol);
    #[cfg(not(feature = "solana"))]
    let usd_per_sol: Option<f64> = None;

    let alpha: u8 = 220;

    let Ok(ctx) = contexts.ctx_mut() else { return };
    paint_black_backdrop(ctx);

    let text_primary = egui::Color32::from_rgba_unmultiplied(240, 240, 240, alpha);
    let text_secondary = egui::Color32::from_rgba_unmultiplied(160, 160, 160, alpha);
    let text_gold = egui::Color32::from_rgba_unmultiplied(244, 187, 68, alpha);
    let text_green = egui::Color32::from_rgba_unmultiplied(80, 220, 120, alpha);
    let text_red = egui::Color32::from_rgba_unmultiplied(255, 100, 100, alpha);
    let text_blue = egui::Color32::from_rgba_unmultiplied(100, 180, 255, alpha);

    let frame = StyledPanel::popup_alpha(alpha);

    let player_color = payout_info.as_ref().and_then(|p| p.player_color);
    let (headline, headline_color) = match (game_over.winner(), player_color) {
        (Some(w), Some(pc)) if w == pc => ("You Won", text_gold),
        (Some(_), Some(_)) => ("You Lost", text_red),
        (None, _) => ("Draw", text_secondary),
        (Some(PieceColor::White), None) => ("White Wins", text_primary),
        (Some(PieceColor::Black), None) => ("Black Wins", text_secondary),
    };

    let termination = game_over.termination_text();

    let mut trigger_dispute = false;
    let mut save_pgn = false;
    let mut play_again_bot = false;
    let mut go_to_bracket = false;
    let mut trigger_rematch = false;

    let is_single_player = matches!(*game_mode, crate::core::GameMode::SinglePlayer);
    let _is_online = matches!(
        *game_mode,
        crate::core::GameMode::OnlineMultiplayer | crate::core::GameMode::MultiplayerCompetitive
    );

    egui::Window::new("")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([380.0, 340.0])
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(348.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    TextStyle::popup_title("GAME OVER")
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha)),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(headline)
                        .size(22.0)
                        .family(egui::FontFamily::Name("CinzelBold".into()))
                        .color(headline_color),
                );
                if !termination.is_empty() {
                    ui.label(
                        egui::RichText::new(termination)
                            .size(13.0)
                            .italics()
                            .color(text_secondary),
                    );
                }

                if let Some(info) = payout_info.as_ref() {
                    if !info.is_rated {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("Practice game — no rating change")
                                .size(11.0)
                                .color(text_secondary),
                        );
                    } else {
                        let delta = info.elo_delta();
                        if info.elo_before > 0 {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                let delta_text = if delta >= 0 {
                                    format!("+{}", delta)
                                } else {
                                    format!("{}", delta)
                                };
                                let delta_color = if delta >= 0 { text_green } else { text_red };
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Rating: {} → {}",
                                        info.elo_before, info.elo_after
                                    ))
                                    .size(13.0)
                                    .color(text_primary),
                                );
                                ui.label(
                                    egui::RichText::new(delta_text)
                                        .size(13.0)
                                        .strong()
                                        .color(delta_color),
                                );
                                if info.win_streak >= 3 {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{}-game streak!",
                                                    info.win_streak
                                                ))
                                                .size(11.0)
                                                .color(text_gold),
                                            );
                                        },
                                    );
                                }
                            });
                            // Tier-up badge
                            let old_tier = elo_tier(info.elo_before);
                            let new_tier = elo_tier(info.elo_after);
                            if new_tier != old_tier {
                                if let Some(tier) = new_tier {
                                    ui.add_space(4.0);
                                    // vertical_centered centers children horizontally;
                                    // horizontal_centered would leave the label left-aligned.
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new(format!("Tier Up!  {}", tier))
                                                .size(14.0)
                                                .strong()
                                                .color(text_gold),
                                        );
                                    });
                                }
                            }
                        }
                    }
                }

                if let Some(info) = payout_info.as_ref() {
                    if info.is_wager_game() {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("Payout Details")
                                .size(13.0)
                                .strong()
                                .color(text_gold),
                        );
                        ui.add_space(6.0);

                        // Fee rows
                        let rows: &[(&str, i64, bool)] = &[
                            ("Pot (2× wager)", (info.wager_amount * 2) as i64, true),
                            ("Treasury fee", -(info.country_fee as i64), false),
                            ("ELO fee", -(info.elo_fee as i64), false),
                            ("Network fee", -(info.operating_cost as i64), false),
                            ("Rent returned", info.rent_return as i64, true),
                        ];
                        for (label, lamports, is_positive) in rows {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(*label).size(11.0).color(text_secondary),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let sign = if *lamports < 0 { "- " } else { "+ " };
                                        let color =
                                            if *is_positive { text_green } else { text_red };
                                        let sol = GameOverPayoutInfo::format_sol_usd(
                                            (*lamports).unsigned_abs(),
                                            usd_per_sol,
                                        );
                                        ui.label(
                                            egui::RichText::new(format!("{}{}", sign, sol))
                                                .size(11.0)
                                                .color(color),
                                        );
                                    },
                                );
                            });
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        let prize_label = if info.is_draw {
                            "Returned:"
                        } else {
                            "You Won:"
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(prize_label)
                                    .size(14.0)
                                    .strong()
                                    .color(text_gold),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // USDC prize path
                                    if let (Some(_mint), Some(usdc)) =
                                        (&info.usdc_mint, info.usdc_prize_usdc)
                                    {
                                        let usdc_fmt =
                                            format!("{:.2} USDC", usdc as f64 / 1_000_000.0);
                                        ui.label(
                                            egui::RichText::new(usdc_fmt)
                                                .size(16.0)
                                                .strong()
                                                .color(text_gold),
                                        );
                                    } else {
                                        ui.label(
                                            egui::RichText::new(
                                                GameOverPayoutInfo::format_sol_usd(
                                                    info.player_winnings(),
                                                    usd_per_sol,
                                                ),
                                            )
                                            .size(16.0)
                                            .strong()
                                            .color(text_gold),
                                        );
                                    }
                                },
                            );
                        });
                        if info.is_draw {
                            if let (Some(_mint), Some(usdc)) =
                                (&info.usdc_mint, info.usdc_prize_usdc)
                            {
                                let usdc_fmt = format!("{:.2} USDC", usdc as f64 / 1_000_000.0);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Both players receive: {}",
                                        usdc_fmt
                                    ))
                                    .size(11.0)
                                    .italics()
                                    .color(text_secondary),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Both players receive: {}",
                                        GameOverPayoutInfo::format_sol_usd(
                                            info.player_winnings(),
                                            usd_per_sol
                                        )
                                    ))
                                    .size(11.0)
                                    .italics()
                                    .color(text_secondary),
                                );
                            }
                        }

                        ui.add_space(4.0);
                        // Settlement status row
                        if info.payout_confirmed {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Prize claimed  ✓")
                                        .size(12.0)
                                        .color(text_green),
                                );
                                if let Some(sig) = &info.finalize_sig {
                                    let short = format!("{}...", &sig[..8.min(sig.len())]);
                                    let url =
                                        format!("https://solscan.io/tx/{}?cluster=devnet", sig);
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .link(
                                                    egui::RichText::new(short)
                                                        .size(11.0)
                                                        .color(text_secondary),
                                                )
                                                .clicked()
                                            {
                                                open_url(&url);
                                            }
                                        },
                                    );
                                }
                            });
                        } else if let Some(reason) = &info.settlement_pending_reason {
                            ui.label(
                                egui::RichText::new(format!("Settlement pending: {reason}."))
                                    .size(11.0)
                                    .italics()
                                    .color(text_secondary),
                            );
                        } else {
                            let elapsed = info.settlement_elapsed();
                            if elapsed < 60 {
                                let dots = match elapsed % 3 {
                                    0 => ".",
                                    1 => "..",
                                    _ => "...",
                                };
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Settling on-chain{} ({}s)",
                                        dots, elapsed
                                    ))
                                    .size(11.0)
                                    .italics()
                                    .color(text_secondary),
                                );
                            } else {
                                let url = info
                                    .finalize_sig
                                    .as_deref()
                                    .map(|s| format!("https://solscan.io/tx/{}?cluster=devnet", s))
                                    .unwrap_or_default();
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("Taking longer than expected — ")
                                            .size(11.0)
                                            .color(text_red),
                                    );
                                    if !url.is_empty()
                                        && ui
                                            .link(
                                                egui::RichText::new("check Solscan")
                                                    .size(11.0)
                                                    .color(text_blue),
                                            )
                                            .clicked()
                                    {
                                        open_url(&url);
                                    }
                                });
                            }
                        }

                        // Dispute button — loser only, within 48 h
                        let is_loser = matches!(
                            (game_over.winner(), player_color),
                            (Some(PieceColor::White), Some(PieceColor::Black))
                                | (Some(PieceColor::Black), Some(PieceColor::White))
                        );
                        if is_loser && info.payout_confirmed && info.dispute_window_open() {
                            ui.add_space(4.0);
                            if let Some(sig) = &info.dispute_sig {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Dispute submitted ({}...)",
                                        &sig[..8.min(sig.len())]
                                    ))
                                    .size(11.0)
                                    .color(text_secondary),
                                );
                            } else if ui
                                .button(egui::RichText::new("Dispute Result").size(12.0))
                                .clicked()
                            {
                                trigger_dispute = true;
                            }
                        }
                    } else {
                        // With no wager pot, the backend bears ER and undelegation costs.
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("Free game — network costs covered by XFChess")
                                .size(11.0)
                                .italics()
                                .color(text_secondary),
                        );
                    }
                }

                ui.add_space(6.0);
                ui.add(egui::Separator::default().shrink(40.0));
                ui.add_space(4.0);

                {
                    let has_rematch = payout_info.as_ref().map(|i| i.game_id > 0).unwrap_or(false);
                    let dark_btn = egui::Color32::from_rgba_unmultiplied(40, 40, 44, 200);
                    let spacing = 6.0_f32;
                    let btn_w = 80.0_f32;
                    let n = if has_rematch { 2 } else { 1 };
                    let total = n as f32 * btn_w + (n - 1) as f32 * spacing;
                    let pad = ((ui.available_width() - total) / 2.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.spacing_mut().item_spacing.x = spacing;
                        let pgn_loading = !cached_pgn.braid_pgn_ready && cached_pgn.pgn.is_some();
                        let save_label = if pgn_loading { "Save ⟳" } else { "Save PGN" };
                        let dim = if pgn_loading {
                            egui::Color32::from_rgba_unmultiplied(40, 40, 44, 120)
                        } else {
                            dark_btn
                        };

                        let resp_save = ui.add_sized(
                            [btn_w, 26.0],
                            egui::Button::new(egui::RichText::new(save_label).size(11.0)).fill(dim),
                        );
                        if pgn_loading {
                            resp_save.on_hover_text("Fetching authoritative game record…");
                        } else if resp_save.clicked() {
                            save_pgn = true;
                        }
                        if has_rematch
                            && ui
                                .add_sized(
                                    [btn_w, 26.0],
                                    egui::Button::new(egui::RichText::new("Rematch").size(11.0))
                                        .fill(dark_btn),
                                )
                                .clicked()
                        {
                            trigger_rematch = true;
                        }
                    });
                }

                ui.add_space(8.0);

                {
                    let spacing = 8.0_f32;
                    let btn_w = 130.0_f32;
                    let total = 2.0 * btn_w + spacing;
                    let pad = ((ui.available_width() - total) / 2.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.spacing_mut().item_spacing.x = spacing;
                        let play_label = if is_single_player {
                            "Play Again"
                        } else {
                            "New Game"
                        };
                        if ui
                            .add_sized(
                                [btn_w, 32.0],
                                egui::Button::new(
                                    egui::RichText::new(play_label)
                                        .size(12.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(20, 18, 10)),
                                )
                                .fill(egui::Color32::from_rgba_unmultiplied(244, 187, 68, 220)),
                            )
                            .clicked()
                        {
                            if is_single_player {
                                play_again_bot = true;
                            } else {
                                next_state.set(GameState::InGame);
                            }
                        }
                        let back_label =
                            if payout_info.as_ref().and_then(|p| p.tournament_id).is_some() {
                                "Back to Bracket"
                            } else {
                                "Main Menu"
                            };
                        if ui
                            .add_sized(
                                [btn_w, 32.0],
                                egui::Button::new(egui::RichText::new(back_label).size(12.0))
                                    .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 44, 200)),
                            )
                            .clicked()
                        {
                            if payout_info.as_ref().and_then(|p| p.tournament_id).is_some() {
                                go_to_bracket = true;
                            } else {
                                next_state.set(GameState::MainMenu);
                            }
                        }
                    });
                }
            });
        });

    if trigger_dispute {
        commands.insert_resource(PendingDispute {
            game_id: 0,
            sig_rx: None,
        });
    }

    if save_pgn {
        let pgn_text = cached_pgn.pgn_string.clone();
        // Save PGNs using this instance's in-memory profile, not the shared disk
        // active-profile field that another local instance may change.
        let dir = match player_identity.username.as_deref() {
            Some(name) => crate::multiplayer::network::identity::profile_pgn_dir(name),
            #[cfg(target_os = "android")]
            None => crate::core::android_paths::external_data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from(".")),
            #[cfg(not(target_os = "android"))]
            None => dirs::document_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("xfchess"),
        };
        let save_result = std::fs::create_dir_all(&dir).and_then(|_| {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let path = dir.join(format!("game_{}.pgn", timestamp));
            std::fs::write(&path, pgn_text).map(|_| path)
        });
        match save_result {
            Ok(path) => {
                popup_queue.push(
                    crate::ui::menus::popup::GamePopup::warning(
                        "PGN Saved",
                        format!("Saved to {}", path.display()),
                    )
                    .with_copy(path.display().to_string()),
                );
            }
            Err(e) => {
                popup_queue.push(crate::ui::menus::popup::GamePopup::warning(
                    "Save Failed",
                    format!("Could not save PGN: {e}"),
                ));
            }
        }
    }

    if play_again_bot {
        *game_mode = crate::core::GameMode::SinglePlayer;
        next_state.set(GameState::InGame);
    }

    if go_to_bracket {
        next_state.set(GameState::MainMenu);
    }

    if trigger_rematch {
        if let (Some(info), Some(rt)) = (payout_info.as_ref(), tokio_runtime.as_ref()) {
            let game_id = info.game_id;
            let base = crate::multiplayer::network::vps::vps_base();
            rt.0.spawn(async move {
                let url = format!("{}/api/games/rematch/{}", base, game_id);
                let _ = reqwest::Client::new().post(&url).send().await;
            });
        }
    }
}

pub fn spectator_game_over_overlay(
    mut contexts: EguiContexts,
    game_over: Res<GameOverState>,
    cached_pgn: Res<CachedGamePgn>,
    mut next_state: ResMut<NextState<GameState>>,
    mut game_mode: ResMut<crate::core::GameMode>,
    mut commands: Commands,
) {
    if !game_over.is_game_over() {
        return;
    }

    let alpha: u8 = 220;

    let Ok(ctx) = contexts.ctx_mut() else { return };
    paint_black_backdrop(ctx);

    let text_p = egui::Color32::from_rgba_unmultiplied(240, 240, 240, alpha);
    let text_s = egui::Color32::from_rgba_unmultiplied(160, 160, 160, alpha);
    let text_g = egui::Color32::from_rgba_unmultiplied(244, 187, 68, alpha);

    let frame = StyledPanel::popup_alpha(alpha);

    let headline = match game_over.winner() {
        Some(PieceColor::White) => ("White Wins!", text_p),
        Some(PieceColor::Black) => ("Black Wins!", text_s),
        None => ("Draw", text_s),
    };
    let termination = game_over.termination_text();
    let pgn_result = match game_over.winner() {
        Some(PieceColor::White) => "1-0",
        Some(PieceColor::Black) => "0-1",
        None => "1/2-1/2",
    };

    let mut trigger_review = false;

    egui::Window::new("spectator_game_over")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([340.0, 200.0])
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(300.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    TextStyle::popup_body("SPECTATING")
                        .color(egui::Color32::from_rgba_unmultiplied(155, 158, 175, alpha)),
                );
                ui.add_space(4.0);
                ui.label(
                    TextStyle::popup_title("GAME OVER")
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha)),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(headline.0)
                        .size(22.0)
                        .family(egui::FontFamily::Name("CinzelBold".into()))
                        .color(headline.1),
                );
                if !termination.is_empty() {
                    ui.label(
                        egui::RichText::new(termination)
                            .size(12.0)
                            .italics()
                            .color(text_s),
                    );
                }
                ui.add_space(16.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    // horizontal_centered only centers vertically — pad to center the pair.
                    ui.add_space(((ui.available_width() - (130.0 + 10.0 + 100.0)) / 2.0).max(0.0));
                    if ui
                        .add_sized(
                            [130.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Review Game").size(13.0).strong(),
                            )
                            .fill(egui::Color32::from_rgb(30, 60, 100)),
                        )
                        .clicked()
                    {
                        trigger_review = true;
                    }
                    if ui
                        .add_sized(
                            [100.0, 38.0],
                            egui::Button::new(egui::RichText::new("Leave").size(13.0))
                                .fill(egui::Color32::from_rgb(60, 30, 30)),
                        )
                        .clicked()
                    {
                        next_state.set(GameState::MainMenu);
                    }
                });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!("Result: {}", pgn_result))
                        .size(10.0)
                        .color(text_g),
                );
            });
        });

    if trigger_review {
        if let Some(pgn) = cached_pgn.pgn.clone() {
            *game_mode = crate::core::GameMode::PgnReplay;
            commands.insert_resource(crate::game::replay::ParsedPgnGameResource {
                inner: pgn,
                show_eval_graph: false,
                puzzle_mode: false,
                puzzle_revealed: false,
            });
            next_state.set(GameState::InGame);
        }
    }
}
