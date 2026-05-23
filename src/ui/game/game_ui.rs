//! In-game UI for chess game display
use crate::core::GameMode;
use crate::game::components::GamePhase;
use crate::game::resources::system_params::GameStateParams;
use crate::rendering::pieces::PieceColor;
use crate::ui::styles::*;
use crate::ui::system_params::GameUIParams;
use bevy::prelude::*;
use bevy_egui::egui;

#[derive(Resource, Default)]
pub struct InGameHudVisibility {
    pub visible: bool,
}

pub fn reset_in_game_hud_visibility(mut hud_visibility: ResMut<InGameHudVisibility>) {
    hud_visibility.visible = true;
}

pub fn toggle_in_game_hud(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hud_visibility: ResMut<InGameHudVisibility>,
) {
    if keyboard.just_pressed(KeyCode::KeyB) {
        hud_visibility.visible = !hud_visibility.visible;
    }
}

pub fn in_game_hud_visible(hud_visibility: Res<InGameHudVisibility>) -> bool {
    hud_visibility.visible
}

/// Main in-game UI: timer, turn indicator, and optional side panel.
pub fn game_status_ui(mut params: GameUIParams) {
    if !params.hud_visibility.visible {
        return;
    }

    let Ok(ctx) = params.contexts.ctx_mut() else {
        return;
    };

    // === FLOATING TIMER ===
    // Skip when no time control is active.
    use crate::game::time_control::TimeControl;
    let tc = params.active_time_control.control;
    let show_timers = !matches!(tc, TimeControl::Unlimited);

    if show_timers {
        let white_active = params.current_turn.color == PieceColor::White;
        let inc = params.game_timer.increment;
        let tc_label = tc.short_label();

        egui::Window::new("floating_timer")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::RIGHT_TOP, [-20.0, 60.0])
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(10.0)
                    .inner_margin(15.0)
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // Mode badge
                    ui.label(
                        egui::RichText::new(tc_label)
                            .size(11.0)
                            .color(egui::Color32::from_rgb(160, 160, 160)),
                    );
                    ui.add_space(4.0);

                    // White timer
                    let white_time = format_time(params.game_timer.white_time_left);
                    let white_secs = params.game_timer.white_time_left;
                    let white_color = if white_active {
                        if white_secs < 10.0 { egui::Color32::from_rgb(255, 80, 80) }
                        else { egui::Color32::WHITE }
                    } else {
                        egui::Color32::from_rgb(140, 140, 140)
                    };
                    ui.label(
                        egui::RichText::new(format!(" {}", white_time))
                            .size(if white_active { 18.0 } else { 15.0 })
                            .color(white_color)
                            .strong(),
                    );

                    ui.add_space(6.0);

                    // Black timer
                    let black_time = format_time(params.game_timer.black_time_left);
                    let black_secs = params.game_timer.black_time_left;
                    let black_active = !white_active;
                    let black_color = if black_active {
                        if black_secs < 10.0 { egui::Color32::from_rgb(255, 80, 80) }
                        else { egui::Color32::WHITE }
                    } else {
                        egui::Color32::from_rgb(140, 140, 140)
                    };
                    ui.label(
                        egui::RichText::new(format!(" {}", black_time))
                            .size(if black_active { 18.0 } else { 15.0 })
                            .color(black_color)
                            .strong(),
                    );

                    if inc > 0.0 {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("+{}s", inc as u32))
                                .size(10.0)
                                .color(egui::Color32::from_rgb(120, 180, 120)),
                        );
                    }
                });
            });
    }

    // === CHECK/CHECKMATE BANNER ===
    // Removed check banner - only show checkmate
    match params.game_state.game_phase.0 {
        GamePhase::Checkmate => render_checkmate_banner(&ctx, &params.game_state),
        _ => {} // No banner for Playing, Stalemate, or Check
    }

    if params.exit_confirmation.visible && !params.game_state.game_over.is_game_over() {
        let is_online = matches!(
            *params.game_mode,
            GameMode::BraidMultiplayer | GameMode::MultiplayerCompetitive
        );
        let confirmation_text = if is_online {
            "Are you sure you want to exit? If you leave an online game, it will be forfeited."
        } else {
            "Are you sure you want to exit this game?"
        };

        egui::Window::new("exit_game_confirmation")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([420.0, 180.0])
            .frame(
                egui::Frame::default()
                    .fill(UiColors::BG_OVERLAY)
                    .corner_radius(12.0)
                    .inner_margin(20.0)
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("EXIT GAME")
                            .size(18.0)
                            .color(UiColors::TEXT_PRIMARY)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(confirmation_text)
                            .size(13.0)
                            .color(UiColors::TEXT_TERTIARY),
                    );
                    ui.add_space(18.0);
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        if ui
                            .add_sized(
                                [120.0, 36.0],
                                egui::Button::new(
                                    egui::RichText::new("No")
                                        .size(13.0)
                                        .color(UiColors::TEXT_PRIMARY)
                                        .strong(),
                                )
                                .fill(UiColors::BG_OVERLAY)
                                .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            params.exit_confirmation.visible = false;
                            params.exit_confirmation.pending_exit = false;
                        }

                        if ui
                            .add_sized(
                                [120.0, 36.0],
                                egui::Button::new(
                                    egui::RichText::new("Yes")
                                        .size(13.0)
                                        .color(UiColors::TEXT_PRIMARY)
                                        .strong(),
                                )
                                .fill(UiColors::DANGER)
                                .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            params.exit_confirmation.pending_exit = true;
                        }
                    });
                });
            });
    }

    // Top bar removed - no turn indicator displayed

    // === ENHANCED GAME INFO PANEL (Left Side) ===
    egui::SidePanel::left("game_info_panel")
        .resizable(false)
        .default_width(220.0)
        .frame(
            egui::Frame::default()
                .fill(UiColors::BG_OVERLAY)
                .inner_margin(15.0)
                .stroke(egui::Stroke::NONE),
        )
        .show(ctx, |ui| {
            ui.add_space(10.0);

            ui.vertical(|ui| {
                // Player Info section
                let is_spectating = *params.game_mode == GameMode::Spectator;
                
                if is_spectating {
                    ui.colored_label(UiColors::ACCENT_GOLD, egui::RichText::new(" SPECTATING").size(16.0).strong());
                    ui.add_space(4.0);
                }
                ui.add_space(8.0);
                
                // Get player names and ELO
                let is_competitive = *params.game_mode == GameMode::MultiplayerCompetitive;
                let (white_name, white_elo, _white_flag, white_sol) = if is_spectating {
                    let w = params.spectator_mode.white_player.as_ref();
                    (
                        format!("{} {}", w.map(|p| country_to_flag(&p.country)).unwrap_or_else(|| "".to_string()), w.map(|p| p.username.clone()).unwrap_or_else(|| "White Player".to_string())),
                        w.map(|p| format!("{} ELO", p.rating)).unwrap_or_default(),
                        "".to_string(),
                        "".to_string()
                    )
                } else if is_competitive {
                    #[cfg(feature = "solana")]
                    {
                        if let (Some(profile), Some(comp)) = (params.solana_profile.as_ref(), params.competitive_match.as_ref()) {
                            // CompetitiveMatchState already carries the wager and opponent metadata.
                            (
                                format!("{} {}", country_to_flag(&profile.country), profile.username),
                                format!("{} ELO", profile.elo),
                                "".to_string(),
                                format!("{:.1} SOL", comp.wager_lamports as f64 / 1_000_000_000.0)
                            )
                        } else {
                            ("White Player".to_string(), "1200 ELO".to_string(), "".to_string(), "0.5 SOL".to_string())
                        }
                    }
                    #[cfg(not(feature = "solana"))]
                    {
                        ("White Player".to_string(), "1200 ELO".to_string(), "".to_string(), "0.5 SOL".to_string())
                    }
                } else {
                    ("White Player".to_string(), "".to_string(), "".to_string(), "".to_string())
                };

                let (black_name, black_elo, _black_flag, black_sol) = if is_spectating {
                    let b = params.spectator_mode.black_player.as_ref();
                    (
                        format!("{} {}", b.map(|p| country_to_flag(&p.country)).unwrap_or_else(|| "".to_string()), b.map(|p| p.username.clone()).unwrap_or_else(|| "Black Player".to_string())),
                        b.map(|p| format!("{} ELO", p.rating)).unwrap_or_default(),
                        "".to_string(),
                        "".to_string()
                    )
                } else if is_competitive {
                    #[cfg(feature = "solana")]
                    {
                        if let (Some(_profile), Some(comp)) = (params.solana_profile.as_ref(), params.competitive_match.as_ref()) {
                            (
                                format!("{} {}", country_to_flag(&comp.opponent_country), comp.opponent_username),
                                format!("{} ELO", comp.opponent_elo),
                                "".to_string(),
                                format!("{:.1} SOL", comp.wager_lamports as f64 / 1_000_000_000.0)
                            )
                        } else {
                            ("Black Player".to_string(), "1180 ELO".to_string(), "".to_string(), "0.5 SOL".to_string())
                        }
                    }
                    #[cfg(not(feature = "solana"))]
                    {
                        ("Black Player".to_string(), "1180 ELO".to_string(), "".to_string(), "0.5 SOL".to_string())
                    }
                } else {
                    ("Black Player".to_string(), "".to_string(), "".to_string(), "".to_string())
                };

                // White Player Info
                render_player_info(ui, &white_name, &white_elo, &white_sol, true);
                ui.add_space(6.0);
                
                // Black Player Info
                render_player_info(ui, &black_name, &black_elo, &black_sol, false);
                
                ui.add_space(12.0);

                // === MATERIAL SCORE BAR ===
                render_material_score_bar(ui, params.game_state.captured.material_advantage());
                
                ui.add_space(15.0);

                // === MOVE HISTORY (Algebraic Notation) ===
                ui.label(
                    egui::RichText::new("MOVE HISTORY")
                        .size(13.0)
                        .color(UiColors::TEXT_TERTIARY)
                        .strong(),
                );
                ui.add_space(8.0);

                render_move_history(ui, &params.move_history);

                // === VIEW MODE TOGGLE ===
                ui.add_space(15.0);
                ui.add_space(12.0);
                
                ui.label(
                    egui::RichText::new("VIEW MODE")
                        .size(11.0)
                        .color(UiColors::TEXT_TERTIARY)
                        .strong(),
                );
                ui.add_space(8.0);
                
                let view_mode_text = match params.view_preferences.local_view {
                    crate::game::view_mode::ViewMode::Standard3D => "3D",
                    crate::game::view_mode::ViewMode::Standard2D => "2D",
                    crate::game::view_mode::ViewMode::TempleOS => "TempleOS",
                };
                
                if ui.add_sized(
                    [ui.available_width(), 32.0],
                    egui::Button::new(
                        egui::RichText::new(format!("Switch to {}", 
                            if view_mode_text == "3D" { "2D" } else { "3D" }))
                            .size(13.0)
                            .color(UiColors::TEXT_PRIMARY)
                            .strong(),
                    )
                    .fill(UiColors::BG_OVERLAY)
                    .stroke(egui::Stroke::NONE),
                ).clicked() {
                    params.view_preferences.toggle_view();
                    *params.view_mode = params.view_preferences.local_view;
                    info!("[UI] View mode toggled to {:?}", params.view_preferences.local_view);
                }
                
                // === EXIT BUTTON (ESC) ===
                ui.add_space(15.0);
                ui.add_space(12.0);
                
                ui.label(
                    egui::RichText::new("ESC to Exit")
                        .size(11.0)
                        .color(UiColors::TEXT_TERTIARY),
                );

                // Game Over Section - Show winner and exit button when game ends
                if params.game_state.game_over.is_game_over() {
                    ui.add_space(20.0);
                    ui.add_space(12.0);

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

                        ui.add_space(12.0);

                        // Exit to Menu button
                        if ui
                            .add_sized(
                                [140.0, 38.0],
                                egui::Button::new(
                                    egui::RichText::new("Exit Game")
                                        .size(14.0)
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
                            if ui.small_button("").on_hover_text("Copy signature").clicked() {
                                ui.output_mut(|o| {
                                    o.commands.push(egui::OutputCommand::CopyText(sig.clone()))
                                });
                            }
                            if ui.small_button("").on_hover_text("Open in explorer").clicked() {
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
                    egui::ScrollArea::vertical()
                        .max_height(f32::INFINITY)
                        .show(ui, |ui| {
                            crate::ui::solana_panel::render_solana_panel(
                                ui,
                                wallet,
                                sync,
                                comp,
                                profile,
                            );
                        });
                });
        }
    }
}

/// Render player information section
fn render_player_info(ui: &mut egui::Ui, name: &str, elo: &str, wager: &str, is_white: bool) {
    let color = if is_white { UiColors::TEXT_PRIMARY } else { UiColors::TEXT_SECONDARY };
    
    ui.colored_label(color, egui::RichText::new(name).size(14.0).strong());
    
    if !elo.is_empty() || !wager.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if !elo.is_empty() {
                ui.label(
                    egui::RichText::new(elo)
                        .size(12.0)
                        .color(UiColors::TEXT_TERTIARY),
                );
            }
            if !elo.is_empty() && !wager.is_empty() {
                ui.add_space(8.0);
            }
            if !wager.is_empty() {
                ui.label(
                    egui::RichText::new(wager)
                        .size(12.0)
                        .color(UiColors::ACCENT_GOLD),
                );
            }
        });
    }
}

/// Render material score bar showing advantage
fn render_material_score_bar(ui: &mut egui::Ui, advantage: i32) {
    ui.label(
        egui::RichText::new("MATERIAL ADVANTAGE")
            .size(11.0)
            .color(UiColors::TEXT_TERTIARY)
            .strong(),
    );
    ui.add_space(6.0);
    
    if advantage == 0 {
        ui.label(
            egui::RichText::new("Even")
                .size(13.0)
                .color(UiColors::TEXT_SECONDARY)
                .strong(),
        );
    } else if advantage > 0 {
        ui.colored_label(
            UiColors::TEXT_PRIMARY,
            egui::RichText::new(format!("White +{}", advantage))
                .size(13.0)
                .strong(),
        );
    } else {
        ui.colored_label(
            UiColors::TEXT_SECONDARY,
            egui::RichText::new(format!("Black +{}", advantage.abs()))
                .size(13.0)
                .strong(),
        );
    }
}

/// Render move history in algebraic notation
fn render_move_history(ui: &mut egui::Ui, history: &crate::game::resources::history::MoveHistory) {
    if history.is_empty() {
        ui.label(
            egui::RichText::new("No moves yet")
                .size(12.0)
                .color(UiColors::TEXT_TERTIARY),
        );
        return;
    }
    
    let moves = &history.moves;
    let mut move_number = 1;
    
    // Display moves in pairs (White then Black)
    for (_i, mv) in moves.iter().enumerate() {
        let is_white = mv.piece_color == PieceColor::White;
        
        if is_white {
            ui.label(
                egui::RichText::new(format!("{}.", move_number))
                    .size(12.0)
                    .color(UiColors::TEXT_TERTIARY)
                    .strong(),
            );
        }
        
        ui.label(
            egui::RichText::new(format_move_algebraic(mv))
                .size(13.0)
                .color(if is_white { UiColors::TEXT_PRIMARY } else { UiColors::TEXT_SECONDARY })
                .strong(),
        );
        
        if is_white {
            ui.add_space(4.0);
        } else {
            ui.add_space(8.0);
            move_number += 1;
        }
    }
}

/// Format a move record as algebraic notation
fn format_move_algebraic(mv: &crate::game::components::MoveRecord) -> String {
    use crate::rendering::pieces::PieceType;
    
    // Piece letter (or empty for pawn)
    let piece_letter = match mv.piece_type {
        PieceType::King => "K",
        PieceType::Queen => "Q",
        PieceType::Rook => "R",
        PieceType::Bishop => "B",
        PieceType::Knight => "N",
        PieceType::Pawn => "",
    };
    
    // Destination square
    let from_file = (b'a' + mv.from.0) as char;
    let _from_rank = mv.from.1 + 1;
    let to_file = (b'a' + mv.to.0) as char;
    let to_rank = mv.to.1 + 1;
    
    // Build notation
    let mut notation = String::new();
    
    // Castling
    if mv.is_castling {
        if mv.to.0 > mv.from.0 {
            notation = "O-O".to_string();
        } else {
            notation = "O-O-O".to_string();
        }
    } else {
        // Normal move
        if mv.piece_type == PieceType::Pawn && mv.captured.is_some() {
            // Pawn capture includes file
            notation.push(from_file);
        } else {
            notation.push_str(piece_letter);
        }
        
        // Capture symbol
        if mv.captured.is_some() {
            notation.push('x');
        }
        
        // Destination
        notation.push(to_file);
        notation.push_str(&to_rank.to_string());
    }
    
    // Check/Checkmate
    if mv.is_checkmate {
        notation.push('#');
    } else if mv.is_check {
        notation.push('+');
    }
    
    notation
}

/// Render a sleek "CHECK" indicator at the top of the screen
#[allow(dead_code)]
fn render_check_banner(ctx: &egui::Context) {
    egui::Window::new("check_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 16.0]) // Slightly higher position
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(173, 92, 47, 200)) // Primary bronze with transparency
                .corner_radius(20.0) // Pill shape
                .inner_margin(egui::Margin::symmetric(16, 8)) // Compact padding
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(244, 187, 68, 150))), // Gold accent border
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // Crown icon for check indication
                ui.label(
                    egui::RichText::new("")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(244, 187, 68)), // Gold color
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("CHECK")
                        .size(13.0)
                        .color(egui::Color32::WHITE)
                        .strong()
                        .extra_letter_spacing(1.0),
                );
            });
        });
}

/// Render a prominent "CHECKMATE!" banner with winner information
fn render_checkmate_banner(ctx: &egui::Context, game_state: &GameStateParams) {
    let (winner_text, winner_color) = match game_state.game_over.winner() {
        Some(PieceColor::White) => ("White Wins!", egui::Color32::from_rgb(240, 240, 240)),
        Some(PieceColor::Black) => ("Black Wins!", egui::Color32::from_rgb(80, 80, 80)),
        None => ("Draw!", egui::Color32::from_rgb(255, 220, 100)),
    };

    egui::Window::new("checkmate_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 20.0]) // Above timer
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(220, 20, 60, 230)) // Red with transparency
                .corner_radius(15.0)
                .inner_margin(25.0)
                .stroke(egui::Stroke::NONE),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("? CHECKMATE!")
                        .size(40.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(winner_text)
                        .size(24.0)
                        .color(winner_color)
                        .strong(),
                );
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new("Game Over")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(255, 200, 200)),
                );
            });
        });
}

/// Format time in seconds to MM:SS format
fn format_time(seconds: f32) -> String {
    let total_seconds = seconds.max(0.0) as u32;
    let minutes = total_seconds / 60;
    let secs = total_seconds % 60;
    format!("{:02}:{:02}", minutes, secs)
}

/// Helper to convert ISO country code to emoji flag
fn country_to_flag(country_code: &str) -> String {
    if country_code.len() != 2 {
        return "".to_string();
    }
    let mut flag = String::new();
    for c in country_code.to_uppercase().chars() {
        let cp = c as u32 + 127397;
        if let Some(ch) = std::char::from_u32(cp) {
            flag.push(ch);
        }
    }
    flag
}

/// Overlay system: shows an Accept/Decline banner when the opponent has offered a draw.
/// Fires [`DrawResponseEvent`] (remote=false) on click so the network layer forwards it.
pub fn draw_offer_ui(
    mut contexts: bevy_egui::EguiContexts,
    pending: Res<crate::game::systems::network_move::PendingDrawOffer>,
    mut draw_response: bevy::prelude::MessageWriter<crate::game::events::DrawResponseEvent>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    let Some(from) = pending.from_player.as_ref() else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("draw_offer_banner")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
        .fixed_size([340.0, 120.0])
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 230))
                .corner_radius(10.0)
                .inner_margin(16.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(80))),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} offers a draw", from))
                        .size(15.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.add_space(12.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

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
                            [120.0, 34.0],
                            egui::Button::new(egui::RichText::new("Accept").color(egui::Color32::WHITE))
                                .fill(egui::Color32::from_rgb(34, 139, 34)),
                        )
                        .clicked()
                    {
                        draw_response.write(crate::game::events::DrawResponseEvent {
                            player: local_player.clone(),
                            accepted: true,
                            remote: false,
                        });
                    }

                    if ui
                        .add_sized(
                            [120.0, 34.0],
                            egui::Button::new(egui::RichText::new("Decline").color(egui::Color32::WHITE))
                                .fill(egui::Color32::from_rgb(180, 40, 40)),
                        )
                        .clicked()
                    {
                        draw_response.write(crate::game::events::DrawResponseEvent {
                            player: local_player,
                            accepted: false,
                            remote: false,
                        });
                    }
                });
            });
        });
}

/// Pause/resume button for online multiplayer: pauses/resumes both clocks.
/// Shown only in BraidMultiplayer mode and only when the game is active.
pub fn pause_resume_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut game_timer: ResMut<crate::game::resources::GameTimer>,
    game_over: Res<crate::game::resources::GameOverState>,
    game_mode: Res<crate::core::GameMode>,
    network_state: Option<Res<crate::multiplayer::BraidNetworkState>>,
    session: Option<Res<crate::multiplayer::network::braid_pvp::BraidPvpSession>>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    use crate::core::GameMode;
    use crate::multiplayer::network::protocol::NetworkMessage;

    if !matches!(*game_mode, GameMode::BraidMultiplayer | GameMode::MultiplayerCompetitive) { return; }
    if game_over.is_game_over() { return; }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let is_paused = !game_timer.is_running;
    let label = if is_paused { "▶ Resume" } else { "⏸ Pause" };
    let color = if is_paused {
        egui::Color32::from_rgb(40, 160, 80)
    } else {
        egui::Color32::from_rgb(160, 120, 40)
    };

    egui::Window::new("pause_resume_btn")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -60.0])
        .fixed_size([100.0, 36.0])
        .frame(egui::Frame::none())
        .show(ctx, |ui| {
            if ui.add_sized([100.0, 34.0],
                egui::Button::new(egui::RichText::new(label).size(13.0).color(egui::Color32::WHITE).strong())
                    .fill(color).corner_radius(6.0),
            ).clicked() {
                let game_id = session.as_ref()
                    .and_then(|s| s.game_id.parse::<u64>().ok())
                    .unwrap_or(0);
                let player = p2p_conn.as_ref()
                    .and_then(|c| c.player_color)
                    .map(|col| match col {
                        PieceColor::White => "white",
                        PieceColor::Black => "black",
                    })
                    .unwrap_or("white")
                    .to_string();

                if is_paused {
                    game_timer.is_running = true;
                    if let Some(ref ns) = network_state {
                        if let Some(ref tx) = ns.message_sender {
                            let _ = tx.send(NetworkMessage::ResumeRequest { game_id, player });
                        }
                    }
                } else {
                    game_timer.is_running = false;
                    if let Some(ref ns) = network_state {
                        if let Some(ref tx) = ns.message_sender {
                            let _ = tx.send(NetworkMessage::PauseRequest { game_id, player });
                        }
                    }
                }
            }
        });
}

/// Overlay system: shows an Accept/Decline banner when the opponent has offered a rematch.
pub fn rematch_offer_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut pending: ResMut<crate::game::systems::network_move::PendingRematchOffer>,
    mut rematch_response: bevy::prelude::MessageWriter<crate::game::events::RematchResponseEvent>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    let Some(from) = pending.from_player.clone() else { return };
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
        .fixed_size([340.0, 120.0])
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 230))
                .corner_radius(10.0)
                .inner_margin(16.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(80))),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} offers a rematch", from))
                        .size(15.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.add_space(12.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    if ui.add_sized([120.0, 34.0],
                        egui::Button::new(egui::RichText::new("Accept").color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(34, 139, 34)),
                    ).clicked() {
                        rematch_response.write(crate::game::events::RematchResponseEvent {
                            player: local_player.clone(),
                            accepted: true,
                            remote: false,
                        });
                        pending.from_player = None;
                    }

                    if ui.add_sized([120.0, 34.0],
                        egui::Button::new(egui::RichText::new("Decline").color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(180, 40, 40)),
                    ).clicked() {
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

/// Post-game evaluation overlay: shown when the game is over with result, reason, and rematch button.
pub fn post_game_overlay(
    mut contexts: bevy_egui::EguiContexts,
    game_over: Res<crate::game::resources::GameOverState>,
    game_mode: Res<crate::core::GameMode>,
    mut next_state: ResMut<NextState<crate::core::GameState>>,
    mut rematch_offer: bevy::prelude::MessageWriter<crate::game::events::RematchOfferEvent>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    move_history: Res<crate::game::resources::MoveHistory>,
) {
    if !game_over.is_game_over() { return; }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let is_online = matches!(*game_mode, crate::core::GameMode::BraidMultiplayer | crate::core::GameMode::MultiplayerCompetitive);

    let result_line = game_over.message();
    let reason_line = match *game_over {
        crate::game::resources::GameOverState::WhiteWon => "by Checkmate",
        crate::game::resources::GameOverState::BlackWon => "by Checkmate",
        crate::game::resources::GameOverState::WhiteWonByResignation => "by Resignation",
        crate::game::resources::GameOverState::BlackWonByResignation => "by Resignation",
        crate::game::resources::GameOverState::WhiteWonByTime => "on Time",
        crate::game::resources::GameOverState::BlackWonByTime => "on Time",
        crate::game::resources::GameOverState::Stalemate => "Stalemate / Draw",
        _ => "",
    };

    let total_moves = move_history.moves.len();

    egui::Window::new("post_game_overlay")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([360.0, 260.0])
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(12, 12, 20, 240))
                .corner_radius(12.0)
                .inner_margin(24.0)
                .stroke(egui::Stroke::new(1.5, egui::Color32::from_gray(70))),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Result header
                let (result_color, icon) = match game_over.winner() {
                    Some(PieceColor::White) => (egui::Color32::from_rgb(220, 220, 220), "♔"),
                    Some(PieceColor::Black) => (egui::Color32::from_rgb(180, 140, 255), "♚"),
                    None => (egui::Color32::GOLD, "="),
                };
                ui.label(egui::RichText::new(icon).size(36.0).color(result_color));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(result_line).size(22.0).color(result_color).strong());
                if !reason_line.is_empty() {
                    ui.label(egui::RichText::new(reason_line).size(14.0).color(egui::Color32::GRAY));
                }
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!("{} moves played", total_moves)).size(12.0).color(egui::Color32::GRAY));

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(12.0);

                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    // Rematch button (online only)
                    if is_online {
                        let local_player = p2p_conn.as_ref()
                            .and_then(|c| c.player_color)
                            .map(|col| match col {
                                PieceColor::White => "white",
                                PieceColor::Black => "black",
                            })
                            .unwrap_or("white")
                            .to_string();

                        if ui.add_sized([120.0, 36.0],
                            egui::Button::new(egui::RichText::new("Rematch").size(14.0).color(egui::Color32::WHITE).strong())
                                .fill(egui::Color32::from_rgb(40, 100, 180)),
                        ).clicked() {
                            rematch_offer.write(crate::game::events::RematchOfferEvent {
                                player: local_player,
                                remote: false,
                            });
                        }
                    }

                    if ui.add_sized([120.0, 36.0],
                        egui::Button::new(egui::RichText::new("Main Menu").size(14.0).color(egui::Color32::WHITE).strong())
                            .fill(egui::Color32::from_rgb(80, 40, 40)),
                    ).clicked() {
                        next_state.set(crate::core::GameState::MainMenu);
                    }
                });
            });
        });
}

