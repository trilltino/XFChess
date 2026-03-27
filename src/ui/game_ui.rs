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

    // Top bar removed - no turn indicator displayed

    // === CAPTURED PIECES PANEL (Left Side) ===
    egui::SidePanel::left("captured_pieces_panel")
        .resizable(false)
        .default_width(140.0)
        .frame(
            egui::Frame::default()
                .fill(UiColors::BG_OVERLAY)
                .inner_margin(10.0)
                .stroke(egui::Stroke::NONE),
        )
        .show(ctx, |ui| {
            ui.add_space(50.0);

            ui.vertical(|ui| {
                render_capture_section(
                    ui,
                    "White Captures",
                    &params.game_state.captured.white_captured,
                    params.game_state.captured.material_advantage(),
                    true,
                );

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                render_capture_section(
                    ui,
                    "Black Captures",
                    &params.game_state.captured.black_captured,
                    -params.game_state.captured.material_advantage(),
                    false,
                );

                // Game Over Section - Show winner and exit button when game ends
                if params.game_state.game_over.is_game_over() {
                    ui.add_space(30.0);
                    ui.separator();
                    ui.add_space(15.0);

                    ui.vertical_centered(|ui| {
                        // Prominent CHECKMATE label
                        if params.game_state.game_over.is_checkmate() {
                            ui.colored_label(
                                UiColors::DANGER,
                                egui::RichText::new("CHECKMATE").size(16.0).strong(),
                            );
                            ui.add_space(6.0);
                        }

                        // Winner declaration
                        let (winner_text, winner_color) = match params.game_state.game_over.winner() {
                            Some(PieceColor::White) => ("White Wins!", UiColors::TEXT_PRIMARY),
                            Some(PieceColor::Black) => ("Black Wins!", UiColors::TEXT_SECONDARY),
                            None => ("Draw!", UiColors::WARNING),
                        };

                        ui.colored_label(
                            winner_color,
                            egui::RichText::new(winner_text).size(18.0).strong(),
                        );

                        ui.add_space(15.0);

                        // Exit to Menu button
                        if ui
                            .add_sized(
                                [120.0, 35.0],
                                egui::Button::new(
                                    egui::RichText::new("Exit Game")
                                        .size(13.0)
                                        .strong(),
                                )
                                .fill(UiColors::DANGER),
                            )
                            .clicked()
                        {
                            params.next_state.set(crate::core::GameState::MainMenu);
                        }
                    });
                }
            });
        });

    // === ON-CHAIN TX PANEL (bottom of left sidebar) ===
    #[cfg(feature = "solana")]
    if let Some(ref txs) = params.recent_txs {
        if !txs.entries.is_empty() {
            egui::SidePanel::left("onchain_tx_panel")
                .resizable(false)
                .default_width(140.0)
                .frame(
                    egui::Frame::default()
                        .fill(UiColors::BG_OVERLAY)
                        .inner_margin(8.0)
                        .stroke(egui::Stroke::NONE),
                )
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("ON-CHAIN MOVES")
                            .size(10.0)
                            .color(UiColors::TEXT_TERTIARY),
                    );
                    ui.add_space(4.0);
                    for (mv, sig) in txs.entries.iter().rev() {
                        let short = format!("{}  …{}", mv, &sig[sig.len().saturating_sub(8)..]);
                        let url = format!(
                            "https://explorer.solana.com/tx/{}?cluster=custom&customUrl=https://devnet-eu.magicblock.app",
                            sig
                        );
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&short)
                                    .size(10.0)
                                    .color(UiColors::SUCCESS),
                            );
                            if ui.small_button("📋").on_hover_text("Copy signature").clicked() {
                                ui.output_mut(|o| {
                                    o.commands.push(egui::OutputCommand::CopyText(sig.clone()))
                                });
                            }
                            if ui.small_button("🔗").on_hover_text("Open in explorer").clicked() {
                                ui.output_mut(|o| {
                                    o.commands.push(egui::OutputCommand::OpenUrl(
                                        egui::OpenUrl::new_tab(url),
                                    ))
                                });
                            }
                        });
                    }
                });
        }
    }

    #[cfg(feature = "solana")]
    if *params.game_mode == GameMode::MultiplayerCompetitive {
        if let (Some(ref mut wallet), Some(ref mut sync), Some(ref mut comp), Some(ref profile)) = (
            params.solana_wallet.as_mut(),
            params.solana_sync.as_mut(),
            params.competitive_match.as_mut(),
            params.solana_profile.as_ref(),
        ) {
            egui::SidePanel::right("solana_sidebar")
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| {
                    crate::ui::solana_panel::render_solana_panel(
                        ui,
                        wallet,
                        sync,
                        comp,
                        &params.players,
                        profile,
                    );
                });
        }
    }
}

/// Unicode chess symbol for a piece type
fn piece_symbol(piece_type: crate::rendering::pieces::PieceType, is_white_section: bool) -> &'static str {
    use crate::rendering::pieces::PieceType;
    if is_white_section {
        // White captured black pieces — show black symbols
        match piece_type {
            PieceType::King => "\u{265A}",
            PieceType::Queen => "\u{265B}",
            PieceType::Rook => "\u{265C}",
            PieceType::Bishop => "\u{265D}",
            PieceType::Knight => "\u{265E}",
            PieceType::Pawn => "\u{265F}",
        }
    } else {
        // Black captured white pieces — show white symbols
        match piece_type {
            PieceType::King => "\u{2654}",
            PieceType::Queen => "\u{2655}",
            PieceType::Rook => "\u{2656}",
            PieceType::Bishop => "\u{2657}",
            PieceType::Knight => "\u{2658}",
            PieceType::Pawn => "\u{2659}",
        }
    }
}

/// Render one side's captured-pieces section.
fn render_capture_section(
    ui: &mut egui::Ui,
    title: &str,
    captures: &[crate::rendering::pieces::PieceType],
    advantage: i32,
    is_white_section: bool,
) {
    use crate::rendering::pieces::PieceType;

    let title_color = if is_white_section {
        UiColors::TEXT_PRIMARY
    } else {
        UiColors::TEXT_SECONDARY
    };

    ui.colored_label(title_color, egui::RichText::new(title).size(14.0).strong());
    ui.add_space(4.0);

    if captures.is_empty() {
        ui.label(
            egui::RichText::new("—")
                .size(12.0)
                .color(UiColors::TEXT_TERTIARY),
        );
    } else {
        // Sorted display order: Queen, Rook, Bishop, Knight, Pawn
        let order = [
            PieceType::Queen,
            PieceType::Rook,
            PieceType::Bishop,
            PieceType::Knight,
            PieceType::Pawn,
        ];

        // Build a single-line tally string: e.g. "♛♜♜♟♟♟"
        let mut tally = String::new();
        for &pt in &order {
            let count = captures.iter().filter(|&&p| p == pt).count();
            for _ in 0..count {
                tally.push_str(piece_symbol(pt, is_white_section));
            }
        }

        ui.label(egui::RichText::new(&tally).size(20.0));

        // Point total
        let total: i32 = captures.iter().map(|p| piece_value(*p)).sum();
        let adv_text = if advantage > 0 {
            format!("{} pts (+{})", total, advantage)
        } else {
            format!("{} pts", total)
        };

        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(adv_text)
                .size(11.0)
                .color(if advantage > 0 {
                    UiColors::SUCCESS
                } else {
                    UiColors::TEXT_TERTIARY
                }),
        );
    }
}

/// Format time in seconds to MM:SS format
fn format_time(seconds: f32) -> String {
    let total_seconds = seconds.max(0.0) as u32;
    let minutes = total_seconds / 60;
    let secs = total_seconds % 60;
    format!("{:02}:{:02}", minutes, secs)
}
