//! Modal popups owned by the main menu.
//!
//! Renders the two purely local-state popups reached from the website-style
//! main menu: the AI setup modal (strength / time-control / side picker that
//! immediately starts a Vs-Computer game) and the controls cheat-sheet opened
//! from the navbar. Both take the bare resources they need so they can be
//! called without the full `MainMenuUIContext`.

use super::*;
use crate::core::{GameMode as CoreGameMode, GameState};
use crate::game::ai::GameMode;
use crate::ui::styles::*;
use bevy::prelude::NextState;
use bevy_egui::egui;
use tracing::info;

/// Render AI setup modal with strength and side selection.
///
/// Clicking a side button applies the current strength + time-control and
/// transitions the game into [`GameState::InGame`] as a single-player AI match.
pub(super) fn render_ai_setup_modal(
    ctx: &egui::Context,
    competitive: &mut CompetitiveMenuState,
    ai_config: &mut crate::game::ai::resource::ChessAIResource,
    core_mode: &mut CoreGameMode,
    next_state: &mut NextState<GameState>,
    active_tc: &mut crate::game::resources::active_time_control::ActiveTimeControl,
) {
    egui::Window::new("Game Setup")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .fixed_size(egui::Vec2::new(380.0, 420.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(TextStyle::popup_title("GAME SETUP"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("✕")
                                    .size(12.0)
                                    .color(UiColors::TEXT_POPUP_BODY),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        )
                        .clicked()
                    {
                        competitive.show_ai_setup = false;
                    }
                });
            });

            ui.add_space(14.0);

            // Strength section
            ui.label(
                egui::RichText::new("Strength")
                    .size(13.0)
                    .color(UiColors::TEXT_POPUP_BODY),
            );
            ui.add_space(6.0);

            // Strength grid (1-8) - compact. Each chip's tooltip explains how
            // that level actually plays (search depth/movetime), not just its
            // ELO number, so the difference between e.g. 1 and 3 is legible.
            ui.horizontal(|ui| {
                for lvl in 1..=8 {
                    let selected = competitive.ai_difficulty == lvl;
                    let response = StyledButton::chip(
                        ui,
                        &format!("{}", lvl),
                        selected,
                        egui::Vec2::new(32.0, 32.0),
                    )
                    .on_hover_text(crate::game::ai::resource::AIDifficulty::from_u8(lvl).tooltip());

                    if response.clicked() {
                        competitive.ai_difficulty = lvl;
                    }
                    ui.add_space(4.0);
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(
                        crate::game::ai::resource::AIDifficulty::from_u8(competitive.ai_difficulty)
                            .description(),
                    )
                    .size(11.0)
                    .color(egui::Color32::from_rgb(150, 150, 150)),
                );
            });

            ui.add_space(16.0);

            // ── Time Control ─────────────────────────────────────────────────
            ui.label(
                egui::RichText::new("Time Control")
                    .size(13.0)
                    .color(UiColors::TEXT_POPUP_BODY),
            );
            ui.add_space(6.0);

            use crate::game::time_control::TimeControl;
            let tc_presets = [
                ("∞", TimeControl::Unlimited),
                ("15s", TimeControl::UltraBullet),
                ("1+0", TimeControl::Bullet),
                ("3+0", TimeControl::BlitzThree),
                ("5+0", TimeControl::Blitz),
                ("10+0", TimeControl::Rapid),
                ("30m", TimeControl::Classical),
            ];
            ui.horizontal_wrapped(|ui| {
                for (label, tc) in tc_presets {
                    let selected = competitive.ai_time_control == tc;
                    let response =
                        StyledButton::chip(ui, label, selected, egui::Vec2::new(44.0, 28.0));
                    if response.clicked() {
                        competitive.ai_time_control = tc;
                    }
                    ui.add_space(3.0);
                }
            });

            ui.add_space(16.0);

            // ── Engine Selection ─────────────────────────────────────────────
            ui.label(
                egui::RichText::new("Engine")
                    .size(13.0)
                    .color(UiColors::TEXT_POPUP_BODY),
            );
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                for (label, engine) in [
                    ("Stockfish", crate::game::ai::resource::AIEngine::Stockfish),
                    (
                        "XFChessEngine",
                        crate::game::ai::resource::AIEngine::XFChessEngine,
                    ),
                ] {
                    let selected = competitive.ai_engine == engine;
                    let response =
                        StyledButton::chip(ui, label, selected, egui::Vec2::new(100.0, 28.0));
                    if response.clicked() {
                        competitive.ai_engine = engine;
                    }
                    ui.add_space(8.0);
                }
            });

            ui.add_space(16.0);

            // Side selection (buttons are self-explanatory)
            ui.add_space(6.0);

            // Side selection — clicking a side immediately starts the game with
            // that side as the player's choice (no separate Play button).
            ui.horizontal(|ui| {
                for (label, side) in [
                    ("Black", AISide::Black),
                    ("Random", AISide::Random),
                    ("White", AISide::White),
                ] {
                    let selected = competitive.ai_side == side;
                    let response =
                        StyledButton::chip(ui, label, selected, egui::Vec2::new(70.0, 40.0));
                    if response.clicked() {
                        competitive.ai_side = side;
                    }
                    ui.add_space(8.0);
                }
            });

            ui.add_space(24.0);

            // ── START GAME BUTTON ───────────────────────────────────────────
            ui.vertical_centered(|ui| {
                let start_btn = egui::Button::new(
                    egui::RichText::new("START GAME")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(egui::Color32::from_rgb(45, 100, 45)) // Greenish start button
                .corner_radius(6.0)
                .min_size(egui::Vec2::new(ui.available_width() * 0.8, 44.0));

                if ui.add(start_btn).clicked() {
                    info!(
                        "[MENU] AI setup modal - START GAME clicked with side: {:?}",
                        competitive.ai_side
                    );
                    ai_config.difficulty =
                        crate::game::ai::resource::AIDifficulty::from_u8(competitive.ai_difficulty);
                    ai_config.mode = GameMode::VsAI {
                        ai_color: match competitive.ai_side {
                            AISide::Black => crate::rendering::pieces::PieceColor::White,
                            AISide::Random => {
                                if rand::random::<bool>() {
                                    crate::rendering::pieces::PieceColor::White
                                } else {
                                    crate::rendering::pieces::PieceColor::Black
                                }
                            }
                            AISide::White => crate::rendering::pieces::PieceColor::Black,
                        },
                    };
                    ai_config.engine = competitive.ai_engine;
                    *core_mode = CoreGameMode::SinglePlayer;
                    active_tc.control = competitive.ai_time_control;
                    active_tc.ai_game = true;
                    next_state.set(GameState::InGame);
                    competitive.show_ai_setup = false;
                }
            });
        });
}

/// Render the controls / keybindings popup reached from the navbar.
pub(super) fn render_controls_popup(ctx: &egui::Context, competitive: &mut CompetitiveMenuState) {
    egui::Window::new("Controls")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::Vec2::new(420.0, 380.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .title_bar(false)
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(TextStyle::popup_title("CONTROLS"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("✕")
                                    .size(12.0)
                                    .color(UiColors::TEXT_POPUP_BODY),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        )
                        .clicked()
                    {
                        competitive.show_controls_popup = false;
                    }
                });
            });

            ui.add_space(14.0);
            ui.add_space(6.0);

            let rows: [(&str, &str); 8] = [
                ("Left Click", "Select piece / confirm move"),
                ("Right Click", "Deselect / cancel"),
                ("Mouse Wheel", "Zoom camera"),
                ("Middle Drag", "Orbit camera"),
                ("Esc", "Pause / back to menu"),
                ("R", "Reset camera view"),
                ("F", "Flip board"),
                ("U", "Undo last move (local only)"),
            ];

            for (key, desc) in rows {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [130.0, 20.0],
                        egui::Label::new(
                            egui::RichText::new(key)
                                .size(13.0)
                                .color(egui::Color32::from_rgb(220, 180, 120))
                                .strong(),
                        ),
                    );
                    ui.label(
                        egui::RichText::new(desc)
                            .size(13.0)
                            .color(egui::Color32::from_rgb(210, 210, 210)),
                    );
                });
                ui.add_space(4.0);
            }
        });
}

/// PGN input modal — paste a PGN string and load it into the replay player.
pub(super) fn render_pgn_input_modal(
    ctx: &egui::Context,
    competitive: &mut CompetitiveMenuState,
    core_mode: &mut CoreGameMode,
    next_state: &mut NextState<GameState>,
    commands: &mut bevy::ecs::system::Commands,
) {
    egui::Window::new("PGN Player")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::Vec2::new(480.0, 380.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .title_bar(false)
        .frame(StyledPanel::popup())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(TextStyle::popup_title("PGN PLAYER"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("✕").size(12.0).color(UiColors::TEXT_POPUP_BODY))
                        .fill(egui::Color32::TRANSPARENT).stroke(egui::Stroke::NONE)).clicked() {
                        competitive.show_pgn_input = false;
                    }
                });
            });

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("Paste PGN below and click Load to replay the game.")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(160, 170, 190)),
            );
            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    ui.add_sized(
                        [440.0, 190.0],
                        egui::TextEdit::multiline(&mut competitive.pgn_input_text)
                            .font(egui::TextStyle::Monospace)
                            .hint_text("[Event \"?\"]\n[White \"Player1\"]\n[Black \"Player2\"]\n\n1. e4 e5 2. Nf3 ..."),
                    );
                });

            if let Some(ref err) = competitive.pgn_input_error.clone() {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("Error: {}", err))
                        .size(10.5)
                        .color(egui::Color32::from_rgb(230, 100, 80)),
                );
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let can_load = !competitive.pgn_input_text.trim().is_empty();
                let load_btn = ui.add_enabled(
                    can_load,
                    egui::Button::new(
                        egui::RichText::new("Load & Play")
                            .size(13.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .fill(egui::Color32::from_rgb(50, 120, 60))
                    .corner_radius(4.0)
                    .min_size(egui::Vec2::new(120.0, 32.0)),
                );

                if load_btn.clicked() {
                    match nimzovich_engine::parse_pgn(&competitive.pgn_input_text) {
                        Ok(pgn) => {
                            info!("[PGN] Loaded game: {} moves", pgn.moves.len());
                            commands.insert_resource(
                                crate::game::replay::ParsedPgnGameResource {
                                    inner: pgn,
                                    show_eval_graph: false,
                                    puzzle_mode: false,
                                    puzzle_revealed: false,
                                },
                            );
                            *core_mode = CoreGameMode::PgnReplay;
                            next_state.set(GameState::InGame);
                            competitive.show_pgn_input = false;
                            competitive.pgn_input_error = None;
                        }
                        Err(e) => {
                            competitive.pgn_input_error = Some(format!("{:?}", e));
                        }
                    }
                }

                ui.add_space(8.0);
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("Cancel").size(13.0))
                            .fill(egui::Color32::from_rgba_unmultiplied(80, 80, 80, 200))
                            .corner_radius(4.0)
                            .min_size(egui::Vec2::new(80.0, 32.0)),
                    )
                    .clicked()
                {
                    competitive.show_pgn_input = false;
                }
            });
        });
}
