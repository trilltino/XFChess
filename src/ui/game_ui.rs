//! In-game UI for chess game display
use crate::core::GameMode;
use crate::game::components::GamePhase;
use crate::game::resources::piece_value;
use crate::rendering::pieces::PieceColor;
use crate::ui::styles::*;
use crate::ui::system_params::GameUIParams;
use bevy::prelude::*;
use bevy_egui::egui;

/// Main in-game UI: timer, turn indicator, and optional side panel.
pub fn game_status_ui(mut params: GameUIParams) {
    let Ok(ctx) = params.contexts.ctx_mut() else {
        return;
    };

    // === FLOATING TIMER ===
    egui::Window::new("floating_timer")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0]) // Centered below top bar
        .frame(
            egui::Frame::default()
                .fill(UiColors::BG_OVERLAY)
                .corner_radius(10.0)
                .inner_margin(15.0)
                .stroke(egui::Stroke::new(1.0, UiColors::BORDER)),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Timer title
                ui.label(
                    egui::RichText::new("GAME TIMER")
                        .size(12.0)
                        .color(UiColors::TEXT_TERTIARY),
                );
                ui.add_space(5.0);

                // White timer
                let white_time = format_time(params.game_timer.white_time_left);
                ui.label(
                    egui::RichText::new(format!("White: {}", white_time))
                        .size(16.0)
                        .color(UiColors::TEXT_PRIMARY)
                        .strong(),
                );

                ui.add_space(5.0);
                ui.separator();
                ui.add_space(5.0);

                // Black timer
                let black_time = format_time(params.game_timer.black_time_left);
                ui.label(
                    egui::RichText::new(format!("Black: {}", black_time))
                        .size(16.0)
                        .color(UiColors::TEXT_PRIMARY)
                        .strong(),
                );
            });
        });

    // Top bar: turn indicator
    egui::TopBottomPanel::top("game_top_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_space(5.0); // Add top padding
            ui.set_min_height(40.0); // Ensure minimum height

            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());

                // Left: Spacer (Timer removed)
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                });

                // Center: Turn Indicator (use available space with manual centering)
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 0.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        if !params.game_state.game_over.is_game_over() {
                            // Get current player
                            let current_player =
                                params.players.current(params.game_state.current_turn.color);
                            let turn_text = format!(
                                "{} ({:?}) to Move",
                                current_player.name, params.game_state.current_turn.color
                            );
                            let turn_color = match params.game_state.current_turn.color {
                                PieceColor::White => UiColors::TEXT_PRIMARY,
                                PieceColor::Black => UiColors::TEXT_SECONDARY,
                            };
                            ui.colored_label(turn_color, egui::RichText::new(turn_text).size(18.0));

                            // Show game phase status
                            match params.game_state.game_phase.0 {
                                GamePhase::Check => {
                                    ui.colored_label(UiColors::DANGER, "CHECK!");
                                }
                                GamePhase::Checkmate => {
                                    ui.colored_label(UiColors::DANGER, "CHECKMATE!");
                                }
                                GamePhase::Stalemate => {
                                    ui.colored_label(UiColors::WARNING, "STALEMATE");
                                }
                                GamePhase::Playing | GamePhase::Setup => {
                                    if params.ai_params.pending_ai.is_some() {
                                        let time = ui.input(|i| i.time);
                                        let dots = (time * 3.0) as i64 % 4;
                                        let text =
                                            format!("AI is thinking{}", ".".repeat(dots as usize));
                                        ui.colored_label(UiColors::INFO, text);
                                    }
                                }
                            }
                        } else {
                            ui.colored_label(
                                UiColors::ACCENT_GOLD,
                                egui::RichText::new(params.game_state.game_over.message())
                                    .size(18.0),
                            );
                        }
                    },
                );

                // Right: Spacer (Settings button removed)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                });
            });
            ui.add_space(5.0);
        });

    // === CAPTURED PIECES PANEL (Left Side) ===
    egui::SidePanel::left("captured_pieces_panel")
        .resizable(false)
        .default_width(120.0)
        .show(ctx, |ui| {
            ui.add_space(60.0); // Space for timer above

            ui.vertical(|ui| {
                // White's captured pieces (pieces White has taken from Black)
                ui.colored_label(
                    UiColors::TEXT_PRIMARY,
                    egui::RichText::new("White Captures").size(14.0).strong(),
                );
                ui.add_space(5.0);

                let white_captures = &params.game_state.captured.white_captured;
                if white_captures.is_empty() {
                    ui.label(
                        egui::RichText::new("None")
                            .size(12.0)
                            .color(UiColors::TEXT_TERTIARY),
                    );
                } else {
                    // Count pieces by type
                    let mut counts = std::collections::HashMap::new();
                    for piece in white_captures {
                        *counts.entry(*piece).or_insert(0) += 1;
                    }

                    // Display counts
                    for (piece_type, count) in counts {
                        let piece_name = format!("{:?}", piece_type);
                        let value = piece_value(piece_type);
                        ui.label(
                            egui::RichText::new(format!("{} x{} ({})", piece_name, count, value))
                                .size(12.0)
                                .color(UiColors::TEXT_SECONDARY),
                        );
                    }

                    // Material advantage
                    let advantage = params.game_state.captured.material_advantage();
                    ui.add_space(5.0);
                    ui.colored_label(
                        if advantage > 0 {
                            UiColors::SUCCESS
                        } else {
                            UiColors::TEXT_TERTIARY
                        },
                        egui::RichText::new(format!("+{} pts", advantage))
                            .size(12.0)
                            .strong(),
                    );
                }

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(10.0);

                // Black's captured pieces (pieces Black has taken from White)
                ui.colored_label(
                    UiColors::TEXT_SECONDARY,
                    egui::RichText::new("Black Captures").size(14.0).strong(),
                );
                ui.add_space(5.0);

                let black_captures = &params.game_state.captured.black_captured;
                if black_captures.is_empty() {
                    ui.label(
                        egui::RichText::new("None")
                            .size(12.0)
                            .color(UiColors::TEXT_TERTIARY),
                    );
                } else {
                    // Count pieces by type
                    let mut counts = std::collections::HashMap::new();
                    for piece in black_captures {
                        *counts.entry(*piece).or_insert(0) += 1;
                    }

                    // Display counts
                    for (piece_type, count) in counts {
                        let piece_name = format!("{:?}", piece_type);
                        let value = piece_value(piece_type);
                        ui.label(
                            egui::RichText::new(format!("{} x{} ({})", piece_name, count, value))
                                .size(12.0)
                                .color(UiColors::TEXT_SECONDARY),
                        );
                    }

                    // Material advantage
                    let advantage = -params.game_state.captured.material_advantage();
                    ui.add_space(5.0);
                    ui.colored_label(
                        if advantage > 0 {
                            UiColors::SUCCESS
                        } else {
                            UiColors::TEXT_TERTIARY
                        },
                        egui::RichText::new(format!("+{} pts", advantage))
                            .size(12.0)
                            .strong(),
                    );
                }
            });
        });

    match *params.game_mode {
        #[cfg(feature = "solana")]
        GameMode::MultiplayerCompetitive => {
            egui::SidePanel::right("solana_sidebar")
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| {
                    crate::ui::solana_panel::render_solana_panel(
                        ui,
                        &mut params.solana_wallet,
                        &mut params.solana_sync,
                        &mut params.competitive_match,
                        &params.players,
                        &params.solana_profile,
                    );
                });
        }
        _ => {
            // Placeholder for other modes
        }
    }
}

/// Format engine score for display
/// Format time in seconds to MM:SS format
fn format_time(seconds: f32) -> String {
    let total_seconds = seconds.max(0.0) as u32;
    let minutes = total_seconds / 60;
    let secs = total_seconds % 60;
    format!("{:02}:{:02}", minutes, secs)
}
