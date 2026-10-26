use super::*;

pub(super) fn render_check_banner(ctx: &egui::Context) {
    egui::Area::new("check_indicator".into())
        .order(egui::Order::Foreground)
        .interactable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 24.0))
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Check")
                    .size(30.0)
                    .family(egui::FontFamily::Name("CinzelBold".into()))
                    .color(egui::Color32::from_rgb(224, 96, 64)),
            );
        });
}

pub(super) fn render_checkmate_banner(
    ctx: &egui::Context,
    game_state: &GameStateParams,
    view_mode: &ViewMode,
    board_rect: Option<egui::Rect>,
) {
    let (winner_text, _) = match game_state.game_over.winner() {
        Some(PieceColor::White) => ("White wins", egui::Color32::from_rgb(240, 240, 240)),
        Some(PieceColor::Black) => ("Black wins", egui::Color32::from_rgb(200, 200, 200)),
        None => ("Draw", egui::Color32::from_rgb(200, 200, 200)),
    };

    let label = format!("Checkmate  \u{2022}  {}", winner_text);

    let mut banner = egui::Window::new("checkmate_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(12, 12, 14, 210))
                .corner_radius(20.0)
                .inner_margin(egui::Margin::symmetric(18, 8))
                .stroke(egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                )),
        );
    if *view_mode == ViewMode::Standard2D {
        if let Some(rect) = board_rect {
            banner = banner
                .pivot(egui::Align2::CENTER_BOTTOM)
                .fixed_pos(egui::pos2(rect.center().x, rect.min.y - 8.0));
        }
    } else {
        banner = banner.anchor(egui::Align2::CENTER_TOP, [0.0, 16.0]);
    }
    banner.show(ctx, |ui| {
        ui.label(
            egui::RichText::new(label)
                .size(13.0)
                .color(egui::Color32::from_rgb(210, 210, 210)),
        );
    });
}

pub fn draw_offer_ui(
    mut contexts: bevy_egui::EguiContexts,
    pending: Res<crate::game::systems::network_move::PendingDrawOffer>,
    mut draw_response: bevy::prelude::MessageWriter<crate::game::events::DrawActionRequest>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    let Some(from) = pending.from_player.as_ref() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("draw_offer_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
        .fixed_size([340.0, 130.0])
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(TextStyle::popup_title(format!("{} offers a draw", from)));
                ui.add_space(12.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    // horizontal_centered only centers vertically — pad to center the pair.
                    ui.add_space(((ui.available_width() - (120.0 * 2.0 + 12.0)) / 2.0).max(0.0));

                    let local_player = p2p_conn
                        .as_ref()
                        .and_then(|c| c.player_color)
                        .map(|col| match col {
                            crate::rendering::pieces::PieceColor::White => "white",
                            crate::rendering::pieces::PieceColor::Black => "black",
                        })
                        .unwrap_or("white")
                        .to_string();

                    if ui
                        .add_sized(
                            [120.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Accept")
                                    .size(13.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(34, 100, 50))
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                        )
                        .clicked()
                    {
                        draw_response.write(crate::game::events::DrawActionRequest {
                            player: local_player.clone(),
                            kind: crate::game::events::DrawActionKind::Accept,
                        });
                    }

                    if ui
                        .add_sized(
                            [120.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Decline")
                                    .size(13.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(UiColors::BTN_POPUP_DARK)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                        )
                        .clicked()
                    {
                        draw_response.write(crate::game::events::DrawActionRequest {
                            player: local_player,
                            kind: crate::game::events::DrawActionKind::Decline,
                        });
                    }
                });
            });
        });
}

pub fn rematch_offer_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut pending: ResMut<crate::game::systems::network_move::PendingRematchOffer>,
    mut rematch_response: bevy::prelude::MessageWriter<crate::game::events::RematchResponseEvent>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    let Some(from) = pending.from_player.clone() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let local_player = p2p_conn
        .as_ref()
        .and_then(|c| c.player_color)
        .map(|col| match col {
            crate::rendering::pieces::PieceColor::White => "white",
            crate::rendering::pieces::PieceColor::Black => "black",
        })
        .unwrap_or("white")
        .to_string();

    egui::Window::new("rematch_offer_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
        .fixed_size([340.0, 130.0])
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(TextStyle::popup_title(format!("{} offers a rematch", from)));
                ui.add_space(12.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    // horizontal_centered only centers vertically — pad to center the pair.
                    ui.add_space(((ui.available_width() - (120.0 * 2.0 + 12.0)) / 2.0).max(0.0));

                    if ui
                        .add_sized(
                            [120.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Accept")
                                    .size(13.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(34, 100, 50))
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                        )
                        .clicked()
                    {
                        rematch_response.write(crate::game::events::RematchResponseEvent {
                            player: local_player.clone(),
                            accepted: true,
                            remote: false,
                        });
                        pending.from_player = None;
                    }

                    if ui
                        .add_sized(
                            [120.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Decline")
                                    .size(13.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(UiColors::BTN_POPUP_DARK)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(8.0),
                        )
                        .clicked()
                    {
                        rematch_response.write(crate::game::events::RematchResponseEvent {
                            player: local_player,
                            accepted: false,
                            remote: false,
                        });
                        pending.from_player = None;
                    }
                });
            });
        });
}

#[derive(Resource, Default)]
pub struct OpponentDisconnectState {
    pub disconnected_at: Option<std::time::Instant>,
}

pub fn ping_chip_ui(
    mut contexts: bevy_egui::EguiContexts,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    game_mode: Res<crate::core::GameMode>,
) {
    use crate::core::GameMode;
    if !matches!(
        *game_mode,
        GameMode::OnlineMultiplayer | GameMode::MultiplayerCompetitive
    ) {
        return;
    }
    let Some(conn) = p2p_conn else { return };
    let Some(rtt) = conn.last_rtt_ms else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let (dot_color, label) = if rtt < 80 {
        (egui::Color32::from_rgb(60, 200, 80), format!("{rtt}ms"))
    } else if rtt < 200 {
        (egui::Color32::from_rgb(230, 180, 40), format!("{rtt}ms"))
    } else {
        (egui::Color32::from_rgb(220, 70, 70), format!("{rtt}ms"))
    };

    egui::Window::new("ping_chip")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -20.0])
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(18, 18, 24, 210))
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(8, 4)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (dot_rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
                ui.painter()
                    .circle_filled(dot_rect.center(), 5.0, dot_color);
                ui.label(
                    egui::RichText::new(label)
                        .size(10.5)
                        .color(egui::Color32::from_gray(200)),
                );
            });
        });
}

pub fn opponent_disconnect_ui(
    mut contexts: bevy_egui::EguiContexts,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    liveness: Res<crate::multiplayer::social::OpponentLivenessState>,
    game_over: Res<crate::game::resources::GameOverState>,
    game_mode: Res<GameMode>,
    mut disc: ResMut<OpponentDisconnectState>,
    first_move_deadline: Res<crate::game::resources::FirstMoveDeadline>,
) {
    use crate::multiplayer::network::p2p::P2PConnectionStatus;
    if game_over.is_game_over() {
        return;
    }

    let is_online = matches!(
        *game_mode,
        GameMode::OnlineMultiplayer | GameMode::MultiplayerCompetitive
    );
    if !is_online {
        return;
    }

    // The first-move grace period owns the pre-game abort path. Showing the
    // disconnect indication at the same time is confusing before move one.
    if first_move_deadline.active {
        disc.disconnected_at = None;
        return;
    }

    // Either a broken established P2P link or missing backend presence marks
    // disconnect. Both must recover before clearing the banner.
    if let Some(conn) = p2p_conn.as_ref() {
        let is_gone = matches!(
            conn.status,
            P2PConnectionStatus::Disconnected | P2PConnectionStatus::Error(_)
        ) || liveness.is_opponent_stale();
        if is_gone {
            if disc.disconnected_at.is_none() {
                disc.disconnected_at = Some(std::time::Instant::now());
            }
        } else if conn.status == P2PConnectionStatus::InGame {
            // Reconnected — clear state
            if disc.disconnected_at.is_some() {
                disc.disconnected_at = None;
            }
        }
    }

    let Some(_since) = disc.disconnected_at else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let col_bg = egui::Color32::from_rgba_unmultiplied(180, 60, 20, 220);
    let col_txt = egui::Color32::WHITE;

    egui::Window::new("opponent_disconnect_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
        .auto_sized()
        .frame(
            egui::Frame::default()
                .fill(col_bg)
                .corner_radius(8.0)
                .inner_margin(12.0),
        )
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Opponent disconnected — reconnecting to your game…")
                    .size(13.0)
                    .color(col_txt)
                    .strong(),
            );
        });
}

pub fn disconnect_recovery_banner(
    mut contexts: bevy_egui::EguiContexts,
    game_mode: Res<crate::core::GameMode>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    game_over: Res<crate::game::resources::GameOverState>,
    mut refresh: bevy::prelude::MessageWriter<
        crate::multiplayer::network::online_game_session::RefreshGameState,
    >,
) {
    let is_online = matches!(
        *game_mode,
        crate::core::GameMode::OnlineMultiplayer | crate::core::GameMode::MultiplayerCompetitive
    );
    if !is_online {
        return;
    }
    if game_over.is_game_over() {
        return;
    }

    let disconnected = p2p_conn
        .as_ref()
        .map(|c| {
            matches!(
                c.status,
                crate::multiplayer::network::p2p::P2PConnectionStatus::Disconnected
                    | crate::multiplayer::network::p2p::P2PConnectionStatus::Error(_)
            )
        })
        .unwrap_or(false);

    if !disconnected {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("disconnect_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 56.0])
        .fixed_size([400.0, 38.0])
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(100, 20, 20, 230))
                .corner_radius(6.0)
                .inner_margin(8.0),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new("Connection lost — attempting to reconnect…")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(255, 180, 180)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Reconnect now").clicked() {
                        refresh.write(
                            crate::multiplayer::network::online_game_session::RefreshGameState,
                        );
                    }
                });
            });
        });
}

pub fn game_over_prompt_banner(
    mut contexts: bevy_egui::EguiContexts,
    state: Res<State<crate::core::GameState>>,
    game_over: Res<crate::game::resources::GameOverState>,
    mut pending: ResMut<crate::game::resources::PendingGameOver>,
    mut next_state: ResMut<NextState<crate::core::GameState>>,
    view_mode: Res<ViewMode>,
    board_layout: Res<crate::ui::game::game_2d::Board2DLayout>,
) {
    if *state.get() != crate::core::GameState::InGame
        || !pending.active
        || !game_over.is_game_over()
    {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Smooth flash (~1.6 s period) between dim and bright.
    let t = ctx.input(|i| i.time);
    let pulse = 0.5 + 0.5 * (t * 4.0).sin();
    let text_alpha = (140.0 + 115.0 * pulse) as u8;
    let border_alpha = (90.0 + 130.0 * pulse) as u8;
    let gold_text = egui::Color32::from_rgba_unmultiplied(244, 187, 68, text_alpha);
    let gold_border = egui::Color32::from_rgba_unmultiplied(244, 187, 68, border_alpha);

    let mut clicked = false;
    let mut prompt = egui::Window::new("game_over_prompt")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(13, 17, 23, 235))
                .stroke(egui::Stroke::new(1.5, gold_border))
                .corner_radius(8.0)
                .inner_margin(12.0),
        );
    if *view_mode == ViewMode::Standard2D {
        if let Some(rect) = board_layout.rect {
            prompt = prompt
                .pivot(egui::Align2::CENTER_TOP)
                .fixed_pos(egui::pos2(rect.center().x, rect.max.y + 8.0));
        }
    } else {
        prompt = prompt.anchor(egui::Align2::CENTER_BOTTOM, [0.0, -96.0]);
    }
    prompt.show(ctx, |ui| {
        let resp = ui.add(
            egui::Button::new(
                egui::RichText::new("Press Enter to see game stats")
                    .size(16.0)
                    .strong()
                    .color(gold_text),
            )
            .frame(false),
        );
        if resp.clicked() {
            clicked = true;
        }
    });

    if clicked {
        info!("[GAME] Game over banner clicked - transitioning to GameOver state");
        pending.active = false;
        next_state.set(crate::core::GameState::GameOver);
    }
}
