use super::*;

pub fn replay_ui_system(
    mut contexts: EguiContexts,
    mut replay: ResMut<PgnReplayState>,
    parsed_pgn: Option<ResMut<ParsedPgnGameResource>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut view_mode: ResMut<ViewMode>,
    game_mode: Res<GameMode>,
    eval_history: Option<Res<crate::ui::game::game_2d::EvalHistory>>,
    mut annotations: ResMut<ReplayAnnotations>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    if *game_mode != GameMode::PgnReplay {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if keyboard.just_pressed(KeyCode::KeyH) {
        replay.show_controls = !replay.show_controls;
    }

    if let Some(rx) = replay.pgn_load_rx.take() {
        match rx.try_recv() {
            Ok(Ok(text)) => {
                replay.is_loading_file = false;
                match xfchess_rules::parse_pgn(&text) {
                    Ok(pgn) => {
                        let snapshots = match pgn.snapshots() {
                            Ok(snapshots) => snapshots,
                            Err(error) => {
                                replay.pgn_input_error = Some(error);
                                return;
                            }
                        };
                        replay.fen_snapshots = snapshots;
                        replay.current_ply = 0;
                        replay.board_ready = false;
                        replay.position_dirty = true;
                        replay.paused = true;
                        replay.pgn_input_error = None;
                        commands.insert_resource(ParsedPgnGameResource {
                            inner: pgn,
                            show_eval_graph: false,
                            puzzle_mode: false,
                            puzzle_revealed: false,
                        });
                        // Resource inserted; it will be visible next frame.
                        return;
                    }
                    Err(e) => {
                        replay.pgn_input_error = Some(format!("{:?}", e));
                    }
                }
            }
            Ok(Err(e)) => {
                replay.is_loading_file = false;
                replay.pgn_input_error = Some(e);
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {
                replay.pgn_load_rx = Some(rx);
            }
            Err(_) => {
                replay.is_loading_file = false;
                replay.pgn_input_error = Some("File loading was interrupted.".to_string());
            }
        }
    }

    // No PGN loaded yet — show the file picker overlay centred on screen.
    if parsed_pgn.is_none() {
        egui::Window::new("pgn_load_overlay")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::Vec2::new(480.0, 280.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(22, 22, 22, 245),
                corner_radius: egui::CornerRadius::same(6),
                stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(60, 60, 60)),
                inner_margin: egui::Margin::same(20),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("PGN Replay")
                            .size(22.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Exit").clicked() {
                            // `cleanup_replay` resets the replay on the way out.
                            next_state.set(GameState::MainMenu);
                        }
                    });
                });
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Choose a .pgn file to replay and analyze.")
                        .size(13.0)
                        .color(egui::Color32::from_rgb(160, 170, 190)),
                );
                ui.add_space(8.0);

                #[cfg(not(target_os = "android"))]
                {
                    if ui
                        .add_enabled(
                            !replay.is_loading_file,
                            egui::Button::new(
                                egui::RichText::new("Choose .pgn File")
                                    .size(15.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(50, 120, 60))
                            .corner_radius(5.0)
                            .min_size(egui::Vec2::new(180.0, 38.0)),
                        )
                        .clicked()
                    {
                        replay.is_loading_file = true;
                        let (tx, rx) = crossbeam_channel::bounded(1);
                        replay.pgn_load_rx = Some(rx);
                        crate::core::net_task::spawn_detached("replay", move || {
                            let result = (|| {
                                let path = rfd::FileDialog::new()
                                    .add_filter("PGN Files", &["pgn"])
                                    .set_title("Choose a PGN file")
                                    .pick_file()
                                    .ok_or("No file selected")?;
                                std::fs::read_to_string(path).map_err(|e| e.to_string())
                            })();
                            let _ = tx.send(result);
                        });
                    }
                }

                #[cfg(target_os = "android")]
                {
                    ui.label(
                        egui::RichText::new("File picker is not available on Android.")
                            .size(13.0)
                            .color(egui::Color32::from_rgb(160, 170, 190)),
                    );
                }

                if let Some(ref err) = replay.pgn_input_error.clone() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("Error: {}", err))
                            .size(10.5)
                            .color(egui::Color32::from_rgb(230, 100, 80)),
                    );
                }
            });
        return;
    }

    let Some(ref pgn) = parsed_pgn else { return };

    let show_graph = pgn.show_eval_graph;
    if show_graph {
        if let Some(eh) = eval_history.as_ref() {
            if !eh.scores.is_empty() {
                egui::Panel::bottom("replay_eval_graph")
                    .exact_size(52.0)
                    .frame(egui::Frame {
                        fill: egui::Color32::from_rgba_unmultiplied(20, 20, 20, 230),
                        inner_margin: egui::Margin::symmetric(8, 4),
                        ..Default::default()
                    })
                    .show(
                        &mut crate::ui::egui_ext::viewport_ui(ctx, "replay_eval_graph"),
                        |ui| {
                            let scores = &eh.scores;
                            let n = scores.len();
                            let avail = ui.available_width();
                            let bar_w = (avail / n as f32).max(2.0).min(12.0);
                            let total_w = bar_w * n as f32;
                            let height = 40.0;
                            let (rect, _) = ui.allocate_exact_size(
                                egui::Vec2::new(total_w, height),
                                egui::Sense::hover(),
                            );
                            let painter = ui.painter();
                            let mid_y = rect.center().y;

                            painter.line_segment(
                                [
                                    egui::Pos2::new(rect.left(), mid_y),
                                    egui::Pos2::new(rect.right(), mid_y),
                                ],
                                egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
                            );

                            for (i, &score) in scores.iter().enumerate() {
                                let Some(score) = score else {
                                    continue;
                                };
                                let x = rect.left() + i as f32 * bar_w;
                                let clamped = score.clamp(-800, 800) as f32;
                                let frac = clamped / 800.0;
                                let bar_h = (frac.abs() * (height / 2.0 - 2.0)).max(1.0);
                                let color = if score >= 0 {
                                    egui::Color32::from_rgb(200, 230, 200)
                                } else {
                                    egui::Color32::from_rgb(80, 80, 80)
                                };
                                let top = if score >= 0 { mid_y - bar_h } else { mid_y };
                                let bot = if score >= 0 { mid_y } else { mid_y + bar_h };
                                painter.rect_filled(
                                    egui::Rect::from_min_max(
                                        egui::Pos2::new(x + 1.0, top),
                                        egui::Pos2::new(x + bar_w - 1.0, bot),
                                    ),
                                    0.0,
                                    color,
                                );
                            }

                            // Current ply marker
                            let cur = replay.current_ply.min(n.saturating_sub(1));
                            let cx = rect.left() + cur as f32 * bar_w + bar_w / 2.0;
                            painter.line_segment(
                                [
                                    egui::Pos2::new(cx, rect.top()),
                                    egui::Pos2::new(cx, rect.bottom()),
                                ],
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255)),
                            );
                        },
                    );
            }
        }
    }

    if replay.show_controls {
        egui::Panel::bottom("replay_controls")
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240),
                inner_margin: egui::Margin::symmetric(12, 8),
                ..Default::default()
            })
            .show(
                &mut crate::ui::egui_ext::viewport_ui(ctx, "replay_controls"),
                |ui| {
                    ui.horizontal(|ui| {
                        // Navigation buttons
                        let btn = |ui: &mut egui::Ui, label: &str| {
                            ui.add_sized(
                                [52.0, 34.0],
                                egui::Button::new(egui::RichText::new(label).size(17.0).strong())
                                    .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
                                    .corner_radius(5.0),
                            )
                        };

                        if btn(ui, "|<<").clicked() {
                            replay.current_ply = 0;
                            replay.position_dirty = true;
                            replay.paused = true;
                            annotations.arrows.clear();
                            annotations.highlights.clear();
                            annotations.dirty = true;
                        }
                        if btn(ui, "<").clicked() {
                            if replay.current_ply > 0 {
                                replay.current_ply -= 1;
                                replay.position_dirty = true;
                            }
                            replay.paused = true;
                            annotations.arrows.clear();
                            annotations.highlights.clear();
                            annotations.dirty = true;
                        }

                        // Play / Pause
                        let play_label = if replay.paused { "▶" } else { "⏸" };
                        if btn(ui, play_label).clicked() {
                            replay.paused = !replay.paused;
                            if !replay.paused {
                                replay.timer = Timer::from_seconds(replay.speed, TimerMode::Once);
                            }
                        }

                        if btn(ui, ">").clicked() {
                            if let Some(ref pgn_res) = parsed_pgn {
                                if replay.current_ply < pgn_res.inner.moves.len() {
                                    replay.current_ply += 1;
                                    replay.position_dirty = true;
                                }
                            }
                            replay.paused = true;
                            annotations.arrows.clear();
                            annotations.highlights.clear();
                            annotations.dirty = true;
                        }
                        if btn(ui, ">>|").clicked() {
                            if let Some(ref pgn_res) = parsed_pgn {
                                replay.current_ply = pgn_res.inner.moves.len();
                                replay.position_dirty = true;
                            }
                            replay.paused = true;
                            annotations.arrows.clear();
                            annotations.highlights.clear();
                            annotations.dirty = true;
                        }

                        ui.add_space(12.0);

                        // 2D/3D toggle
                        let view_label = match *view_mode {
                            ViewMode::Standard2D => "3D",
                            ViewMode::Standard3D => "2D",
                            #[cfg(feature = "templeos")]
                            ViewMode::TempleOS => "3D",
                        };
                        if ui
                            .add_sized(
                                [60.0, 34.0],
                                egui::Button::new(
                                    egui::RichText::new(view_label).size(14.0).strong(),
                                )
                                .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
                                .corner_radius(5.0),
                            )
                            .clicked()
                        {
                            view_mode.toggle();
                            annotations.dirty = true;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_sized(
                                    [120.0, 34.0],
                                    egui::Button::new(
                                        egui::RichText::new("Exit to Menu").size(13.0).strong(),
                                    )
                                    .fill(egui::Color32::from_rgb(120, 70, 70))
                                    .corner_radius(5.0),
                                )
                                .clicked()
                            {
                                next_state.set(GameState::MainMenu);
                            }

                            if ui
                                .add_sized(
                                    [100.0, 30.0],
                                    egui::Button::new(
                                        egui::RichText::new("Hide H").size(12.0).strong(),
                                    )
                                    .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
                                    .corner_radius(4.0),
                                )
                                .clicked()
                            {
                                replay.show_controls = false;
                            }
                        });
                    });
                },
            );
    }

    let Some(pgn) = parsed_pgn else { return };
    let total = pgn.inner.moves.len();
    if total == 0 {
        return;
    }
    // Show the full PGN move list without puzzle gating.
    let visible_total = total;

    egui::Panel::right("replay_move_list")
        .min_size(300.0)
        .max_size(380.0)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(18, 18, 24, 230),
            inner_margin: egui::Margin::symmetric(12, 10),
            ..Default::default()
        })
        .show(
            &mut crate::ui::egui_ext::viewport_ui(ctx, "replay_move_list"),
            |ui| {
                // PGN header (Lichess-style: White / Black / Result)
                if let Some(white) = pgn.inner.tag("White") {
                    ui.label(
                        egui::RichText::new(format!("♔ {}", white))
                            .size(14.0)
                            .color(egui::Color32::from_gray(220)),
                    );
                }
                if let Some(black) = pgn.inner.tag("Black") {
                    ui.label(
                        egui::RichText::new(format!("♚ {}", black))
                            .size(14.0)
                            .color(egui::Color32::from_gray(160)),
                    );
                }
                if !pgn.inner.result.is_empty() {
                    ui.label(
                        egui::RichText::new(&pgn.inner.result)
                            .size(15.0)
                            .color(egui::Color32::GOLD)
                            .strong(),
                    );
                }
                ui.add(egui::Separator::default().spacing(8.0));

                // Move list — Lichess 3-column grid: index | white | black
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        egui::Grid::new("replay_move_grid")
                            .num_columns(3)
                            .min_col_width(30.0)
                            .spacing([4.0, 3.0])
                            .show(ui, |ui| {
                                for move_num in 1..=visible_total.div_ceil(2) {
                                    let white_idx = (move_num - 1) * 2;
                                    let black_idx = white_idx + 1;

                                    // Index column
                                    ui.label(
                                        egui::RichText::new(format!("{}.", move_num))
                                            .size(13.0)
                                            .color(egui::Color32::GRAY),
                                    );

                                    // White move
                                    if white_idx < visible_total {
                                        let is_current = replay.current_ply == white_idx + 1;
                                        let color = if is_current {
                                            egui::Color32::from_rgb(100, 200, 255)
                                        } else {
                                            egui::Color32::WHITE
                                        };
                                        let resp = ui.selectable_label(
                                            is_current,
                                            egui::RichText::new(&pgn.inner.moves[white_idx])
                                                .size(14.0)
                                                .color(color)
                                                .strong(),
                                        );
                                        if resp.clicked() {
                                            replay.current_ply = white_idx + 1;
                                            replay.position_dirty = true;
                                            replay.paused = true;
                                        }
                                    } else {
                                        ui.label("");
                                    }

                                    // Black move
                                    if black_idx < visible_total {
                                        let is_current = replay.current_ply == black_idx + 1;
                                        let color = if is_current {
                                            egui::Color32::from_rgb(100, 200, 255)
                                        } else {
                                            egui::Color32::from_gray(180)
                                        };
                                        let resp = ui.selectable_label(
                                            is_current,
                                            egui::RichText::new(&pgn.inner.moves[black_idx])
                                                .size(14.0)
                                                .color(color)
                                                .strong(),
                                        );
                                        if resp.clicked() {
                                            replay.current_ply = black_idx + 1;
                                            replay.position_dirty = true;
                                            replay.paused = true;
                                        }
                                    } else if white_idx < visible_total {
                                        ui.label(
                                            egui::RichText::new("…")
                                                .size(14.0)
                                                .color(egui::Color32::DARK_GRAY),
                                        );
                                    } else {
                                        ui.label("");
                                    }

                                    ui.end_row();
                                }
                            });

                        ui.add_space(6.0);

                        ui.label(
                            egui::RichText::new(format!("Ply {}/{}", replay.current_ply, total))
                                .size(12.0)
                                .color(egui::Color32::DARK_GRAY),
                        );
                    });
            },
        );
}
