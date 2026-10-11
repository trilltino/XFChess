use super::*;

#[cfg(feature = "solana")]
pub fn tournament_sidebar_widget(
    mut contexts: bevy_egui::EguiContexts,
    tournament: Option<Res<crate::multiplayer::solana::tournament::TournamentClientState>>,
    lobby: Option<Res<crate::states::tournament_menu::TournamentLobbyState>>,
    ai_config: Option<Res<crate::game::ai::resource::ChessAIResource>>,
) {
    let Some(tc) = tournament else { return };
    if tc.active_tournament_id.is_none() {
        return;
    };
    // A tournament ID may outlive participation; suppress tournament status
    // while playing an unrelated AI game.
    if let Some(ai_config) = &ai_config {
        if matches!(
            ai_config.mode,
            crate::game::ai::resource::AiMode::VsAI { .. }
        ) {
            return;
        }
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    let bg = egui::Color32::from_rgba_unmultiplied(12, 20, 30, 210);
    let border = egui::Color32::from_rgba_unmultiplied(180, 140, 0, 180);

    egui::Window::new("tournament_widget")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_TOP, [-12.0, 50.0])
        .auto_sized()
        .frame(
            egui::Frame::default()
                .fill(bg)
                .stroke(egui::Stroke::new(1.0, border))
                .corner_radius(8.0)
                .inner_margin(10.0),
        )
        .show(ctx, |ui| {
            ui.set_max_width(180.0);
            ui.label(
                egui::RichText::new("TOURNAMENT")
                    .size(10.0)
                    .strong()
                    .color(egui::Color32::GOLD),
            );
            ui.add_space(4.0);

            if tc.waiting_for_next_match {
                ui.label(
                    egui::RichText::new("Waiting for next round…")
                        .size(11.0)
                        .italics()
                        .color(egui::Color32::from_rgb(100, 200, 255)),
                );
            }
            if let Some(ref result) = tc.last_match_result {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Last result:")
                            .size(10.0)
                            .color(egui::Color32::from_gray(160)),
                    );
                    let col = match result.as_str() {
                        "1-0" => egui::Color32::from_rgb(100, 220, 100),
                        "0-1" => egui::Color32::from_rgb(220, 100, 100),
                        _ => egui::Color32::from_rgb(220, 200, 100),
                    };
                    ui.label(egui::RichText::new(result).size(11.0).strong().color(col));
                });
            }
            if let Some(slot) = tc.my_slot {
                ui.label(
                    egui::RichText::new(format!("Slot {}", slot + 1))
                        .size(10.0)
                        .color(egui::Color32::from_gray(140)),
                );
            }
            if !tc.status_message.is_empty() {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(&tc.status_message)
                        .size(10.0)
                        .color(egui::Color32::from_gray(180))
                        .italics(),
                );
            }

            // Swiss standings (collapsible)
            if let Some(lobby_state) = lobby.as_ref() {
                if let Some(standings) = &lobby_state.swiss_standings {
                    if !standings.is_empty() {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);
                        let round_label = match (
                            lobby_state.swiss_current_round,
                            lobby_state.swiss_total_rounds,
                        ) {
                            (Some(cur), Some(tot)) => format!("Round {}/{}", cur, tot),
                            (Some(cur), None) => format!("Round {}", cur),
                            _ => "Standings".to_string(),
                        };
                        ui.label(
                            egui::RichText::new(round_label)
                                .size(10.0)
                                .strong()
                                .color(egui::Color32::from_gray(200)),
                        );
                        ui.add_space(2.0);
                        egui::Grid::new("swiss_standings_grid")
                            .num_columns(3)
                            .spacing([4.0, 2.0])
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("Player")
                                        .size(9.0)
                                        .color(egui::Color32::from_gray(140)),
                                );
                                ui.label(
                                    egui::RichText::new("Pts")
                                        .size(9.0)
                                        .color(egui::Color32::from_gray(140)),
                                );
                                ui.label(
                                    egui::RichText::new("BH")
                                        .size(9.0)
                                        .color(egui::Color32::from_gray(140)),
                                );
                                ui.end_row();
                                for (rank, s) in standings.iter().enumerate().take(8) {
                                    let name_short = if s.player.len() > 10 {
                                        format!("{}.", &s.player[..9])
                                    } else {
                                        s.player.clone()
                                    };
                                    let row_col = if rank == 0 {
                                        egui::Color32::GOLD
                                    } else {
                                        egui::Color32::from_gray(200)
                                    };
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}. {}",
                                            rank + 1,
                                            name_short
                                        ))
                                        .size(9.0)
                                        .color(row_col),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{:.1}", s.score as f32 / 2.0))
                                            .size(9.0)
                                            .color(row_col),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{:.1}",
                                            s.buchholz as f32 / 10.0
                                        ))
                                        .size(9.0)
                                        .color(egui::Color32::from_gray(150)),
                                    );
                                    ui.end_row();
                                }
                            });

                        // Round progress dots
                        if let (Some(cur), Some(tot)) = (
                            lobby_state.swiss_current_round,
                            lobby_state.swiss_total_rounds,
                        ) {
                            let tot = tot as usize;
                            let cur = cur as usize;
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                for r in 1..=tot {
                                    let done = r < cur;
                                    let active = r == cur;
                                    let size = egui::Vec2::splat(8.0);
                                    let (dot_rect, _) =
                                        ui.allocate_exact_size(size, egui::Sense::hover());
                                    let center = dot_rect.center();
                                    let p = ui.painter();
                                    if done {
                                        p.circle_filled(center, 4.0, egui::Color32::GOLD);
                                    } else if active {
                                        p.circle_filled(
                                            center,
                                            4.0,
                                            egui::Color32::from_rgb(100, 200, 255),
                                        );
                                    } else {
                                        p.circle_stroke(
                                            center,
                                            3.5,
                                            egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
                                        );
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                        }
                    }
                }
            }
        });
}
