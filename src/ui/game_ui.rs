//! In-game UI for chess game display
//!
//! Provides a comprehensive chess game UI with:
//! - Live move notation (Standard Algebraic Notation)
//! - Game timer for both players
//! - Turn indicator
//! - Settings button
//! - Captured pieces display
//!
//! # Bevy Egui Patterns
//!
//! This module follows bevy_egui best practices:
//! - Uses `EguiContexts` for context access
//! - Returns `Result` to handle context unavailability
//! - Uses SystemParam groups for cleaner APIs
//! - Follows egui layout patterns (TopBottomPanel, SidePanel, etc.)
//!
//! # Error Handling
//!
//! UI systems return `Result<(), QuerySingleError>` to gracefully handle
//! cases where the egui context may not be available (e.g., during state
//! transitions). The wrapper function in `game::plugin` handles these errors.

use crate::core::GameState;
use crate::game::components::{GamePhase, MoveRecord};
use crate::rendering::pieces::{PieceColor, PieceType};
use crate::ui::styles::*;
use crate::ui::system_params::GameUIParams;
use bevy::prelude::*;
use bevy_egui::egui;

/// System to display comprehensive chess game UI
///
/// Renders the main in-game UI including:
/// - Top bar: Game timer, turn indicator, settings button
/// - Right panel: Move history with algebraic notation
/// - Bottom panel: Captured pieces display
///
/// # Execution
///
/// Runs in `EguiPrimaryContextPass` schedule, which ensures it runs
/// after input processing but before rendering.
///
/// # Error Handling
///
/// Returns `Result` to handle cases where the egui context may not be
/// available (e.g., during state transitions). The wrapper function
/// in `game::plugin` handles these errors gracefully.
///
/// # System Parameters
///
/// Uses [`GameUIParams`] to group all UI-related resources into a single parameter,
/// following bevy_egui best practices for cleaner APIs.
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
                // Centered Timer Title
                ui.label(
                    egui::RichText::new("GAME TIMER")
                        .size(12.0)
                        .color(UiColors::TEXT_TERTIARY),
                );
                ui.add_space(5.0);

                // White Timer
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

                // Black Timer
                let black_time = format_time(params.game_timer.black_time_left);
                ui.label(
                    egui::RichText::new(format!("Black: {}", black_time))
                        .size(16.0)
                        .color(UiColors::TEXT_PRIMARY)
                        .strong(),
                );
            });
        });

    // === TOP BAR: Turn, Settings ===

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

                // Right: Settings Button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    if ui.button("Settings").clicked() {
                        params.previous_state.state = GameState::InGame;
                        params.next_state.set(GameState::Settings);
                    }
                });
            });
            ui.add_space(5.0); // Add bottom padding
        });

    // Move Notation Panel removed per user request
    /*
    egui::SidePanel::left("move_notation_panel")
        .resizable(true)
        .default_width(250.0)
        .min_width(200.0)
        .max_width(400.0)
        .show(ctx, |ui| {
             // ... content ...
        });
    */

    // Captured Pieces Panel removed per user request
    /*
    egui::SidePanel::right("captured_pieces_panel")
        .resizable(true)
        .default_width(200.0)
        .min_width(150.0)
        .max_width(300.0)
        .show(ctx, |ui| {
            // ... content ...
        });
    */

    // === AI Statistics Panel (Bottom Left) - only show when playing vs AI ===
    // === AI Statistics Panel (Bottom Left) - only show when playing vs AI ===
    // Note: Currently GameMode only has VsAI, but we use match for future extensibility
    // AI Stats Panel removed per user request
    /*
    match params.ai_params.ai_config.mode {
        GameMode::VsAI { .. } => {
             // ... content ...
        }
    }
    */
}

/// Format engine score for display
#[allow(dead_code)] // May be used when move notation panel is re-enabled
fn format_score(score: i64) -> String {
    if score.abs() > 9000 {
        // Checkmate score
        if score > 0 {
            "M (White winning)".to_string()
        } else {
            "M (Black winning)".to_string()
        }
    } else {
        // Centipawn score (divide by 100 for pawns)
        let pawns = score as f64 / 100.0;
        if pawns > 0.0 {
            format!("+{:.2}", pawns)
        } else {
            format!("{:.2}", pawns)
        }
    }
}

/// Format node count with commas
#[allow(dead_code)] // May be used when move notation panel is re-enabled
fn format_nodes(nodes: i64) -> String {
    let s = nodes.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}

/// Convert piece type to text abbreviation
#[allow(dead_code)] // May be used when move notation panel is re-enabled
fn piece_unicode(piece_type: PieceType, color: PieceColor) -> String {
    let symbol = match (color, piece_type) {
        (PieceColor::White, PieceType::King) => "K",
        (PieceColor::White, PieceType::Queen) => "Q",
        (PieceColor::White, PieceType::Rook) => "R",
        (PieceColor::White, PieceType::Bishop) => "B",
        (PieceColor::White, PieceType::Knight) => "N",
        (PieceColor::White, PieceType::Pawn) => "P",
        (PieceColor::Black, PieceType::King) => "k",
        (PieceColor::Black, PieceType::Queen) => "q",
        (PieceColor::Black, PieceType::Rook) => "r",
        (PieceColor::Black, PieceType::Bishop) => "b",
        (PieceColor::Black, PieceType::Knight) => "n",
        (PieceColor::Black, PieceType::Pawn) => "p",
    };

    format!("{} ", symbol)
}

/// Convert MoveRecord to Standard Algebraic Notation (SAN)
///
/// Examples:
/// - e4, Nf3, O-O, O-O-O
/// - exd5, Nxe5, Qxf7+
/// - e8=Q, f8=N#
#[allow(dead_code)] // May be used when move notation panel is re-enabled
fn move_to_notation(move_record: &MoveRecord) -> String {
    // Castling notation
    if move_record.is_castling {
        // Determine kingside (O-O) or queenside (O-O-O)
        let kingside = move_record.to.0 > move_record.from.0;
        return if kingside {
            "O-O".to_string()
        } else {
            "O-O-O".to_string()
        };
    }

    // Get piece symbol
    let piece_symbol = match move_record.piece_type {
        PieceType::King => "K",
        PieceType::Queen => "Q",
        PieceType::Rook => "R",
        PieceType::Bishop => "B",
        PieceType::Knight => "N",
        PieceType::Pawn => "", // Pawns don't have a symbol
    };

    // Convert coordinates to chess notation (a-h, 1-8)
    let from_square = square_to_notation(move_record.from);
    let to_square = square_to_notation(move_record.to);

    // Build notation
    let mut notation = String::new();

    // Add piece symbol (if not pawn)
    if !piece_symbol.is_empty() {
        notation.push_str(piece_symbol);
    }

    // For pawns, add source file if capturing
    if move_record.piece_type == PieceType::Pawn && move_record.captured.is_some() {
        if let Some(first_char) = from_square.chars().next() {
            notation.push(first_char);
        }
    }

    // Add capture indicator
    if move_record.captured.is_some() || move_record.is_en_passant {
        notation.push('x');
    }

    // Add destination square
    notation.push_str(&to_square);

    // Add promotion notation (if applicable)
    // Note: MoveRecord doesn't currently track promotion piece, so we'll skip this for now
    // In a full implementation, you'd check if pawn reached 8th rank and add =Q, =N, etc.

    // Add check/checkmate indicators
    if move_record.is_checkmate {
        notation.push('#');
    } else if move_record.is_check {
        notation.push('+');
    }

    // Add en passant notation
    if move_record.is_en_passant {
        notation.push_str(" e.p.");
    }

    notation
}

/// Convert (x, y) coordinates to chess square notation (e.g., (4, 0) -> "e1")
#[allow(dead_code)] // May be used when move notation panel is re-enabled
fn square_to_notation(pos: (u8, u8)) -> String {
    let file = (b'a' + pos.0) as char;
    let rank = (b'1' + pos.1) as char;
    format!("{}{}", file, rank)
}

/// Format time in seconds to MM:SS format
fn format_time(seconds: f32) -> String {
    let total_seconds = seconds.max(0.0) as u32;
    let minutes = total_seconds / 60;
    let secs = total_seconds % 60;
    format!("{:02}:{:02}", minutes, secs)
}
