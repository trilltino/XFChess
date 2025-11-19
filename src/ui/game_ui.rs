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
use crate::game::ai::GameMode;
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
pub fn game_status_ui(
    mut params: GameUIParams,
) -> Result<(), bevy::ecs::query::QuerySingleError> {
    let ctx = params.contexts.ctx_mut()?;

    // === TOP BAR: Timer, Turn, Settings ===
    egui::TopBottomPanel::top("game_top_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());

                // Left: Game Timer
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add_space(10.0);

                    // White Timer
                    let white_time = format_time(params.game_timer.white_time_left);
                    let white_color = if params.game_state.current_turn.color == PieceColor::White
                        && params.game_timer.is_running
                    {
                        egui::Color32::from_rgb(255, 255, 100) // Highlight active player
                    } else {
                        egui::Color32::WHITE
                    };
                    ui.colored_label(white_color, format!("White: {}", white_time));

                    ui.add_space(20.0);

                    // Black Timer
                    let black_time = format_time(params.game_timer.black_time_left);
                    let black_color = if params.game_state.current_turn.color == PieceColor::Black
                        && params.game_timer.is_running
                    {
                        egui::Color32::from_rgb(255, 255, 100) // Highlight active player
                    } else {
                        egui::Color32::WHITE
                    };
                    ui.colored_label(black_color, format!("Black: {}", black_time));
                });

                // Center: Turn Indicator (use available space with manual centering)
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 0.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        if !params.game_state.game_over.is_game_over() {
                            // Get current player
                            let current_player = params.players.current(params.game_state.current_turn.color);
                            let turn_text = format!(
                                "{} ({:?}) to Move",
                                current_player.name, params.game_state.current_turn.color
                            );
                            let turn_color = match params.game_state.current_turn.color {
                                PieceColor::White => egui::Color32::from_rgb(240, 240, 240),
                                PieceColor::Black => egui::Color32::from_rgb(50, 50, 50),
                            };
                            ui.colored_label(turn_color, egui::RichText::new(turn_text).size(18.0));

                            // Show game phase status
                            match params.game_state.game_phase.0 {
                                GamePhase::Check => {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(255, 100, 100),
                                        "CHECK!",
                                    );
                                }
                                GamePhase::Checkmate => {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(255, 0, 0),
                                        "CHECKMATE!",
                                    );
                                }
                                GamePhase::Stalemate => {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(200, 200, 0),
                                        "STALEMATE",
                                    );
                                }
                                GamePhase::Playing | GamePhase::Setup => {
                                    if params.ai_params.pending_ai.is_some() {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(100, 150, 255),
                                            "AI is thinking...",
                                        );
                                    }
                                }
                            }
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 200, 0),
                                egui::RichText::new(params.game_state.game_over.message()).size(18.0),
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
        });

    // === LEFT SIDE: Move Notation Panel ===
    egui::SidePanel::left("move_notation_panel")
        .resizable(true)
        .default_width(250.0)
        .min_width(200.0)
        .max_width(400.0)
        .show(ctx, |ui| {
            ui.heading(TextStyle::heading("Move Notation", TextSize::MD));
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    if params.move_history.moves.is_empty() {
                        ui.label(TextStyle::caption("No moves yet. Make your first move!"));
                    } else {
                        // Group moves by move number (White + Black = 1 move)
                        let mut move_num = 1;
                        let mut i = 0;

                        while i < params.move_history.moves.len() {
                            ui.horizontal(|ui| {
                                // Move number
                                ui.label(TextStyle::body(format!("{}.", move_num)));

                                // White's move
                                if i < params.move_history.moves.len() {
                                    let white_move = &params.move_history.moves[i];
                                    let notation = move_to_notation(white_move);
                                    ui.label(TextStyle::body(notation));
                                    i += 1;
                                }

                                // Black's move
                                if i < params.move_history.moves.len() {
                                    let black_move = &params.move_history.moves[i];
                                    let notation = move_to_notation(black_move);
                                    ui.label(TextStyle::body(notation));
                                    i += 1;
                                }
                            });

                            move_num += 1;
                        }
                    }
                });
        });

    // === RIGHT SIDE: Captured Pieces Panel ===
    egui::SidePanel::right("captured_pieces_panel")
        .resizable(true)
        .default_width(200.0)
        .min_width(150.0)
        .max_width(300.0)
        .show(ctx, |ui| {
            ui.heading(TextStyle::heading("Captured Pieces", TextSize::MD));
            ui.separator();

            ui.vertical(|ui| {
                // White's captures (black pieces taken)
                ui.label(TextStyle::body("White Captured:"));
                ui.horizontal_wrapped(|ui| {
                    if params.game_state.captured.white_captured.is_empty() {
                        ui.label(TextStyle::caption("None"));
                    } else {
                        for piece in &params.game_state.captured.white_captured {
                            ui.label(piece_unicode(*piece, PieceColor::Black));
                        }
                    }
                });

                ui.add_space(10.0);

                // Black's captures (white pieces taken)
                ui.label(TextStyle::body("Black Captured:"));
                ui.horizontal_wrapped(|ui| {
                    if params.game_state.captured.black_captured.is_empty() {
                        ui.label(TextStyle::caption("None"));
                    } else {
                        for piece in &params.game_state.captured.black_captured {
                            ui.label(piece_unicode(*piece, PieceColor::White));
                        }
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(5.0);

                // Material advantage
                let advantage = params.game_state.captured.material_advantage();
                if advantage > 0 {
                    ui.label(TextStyle::body(format!("White: +{}", advantage)));
                } else if advantage < 0 {
                    ui.label(TextStyle::body(format!("Black: +{}", -advantage)));
                } else {
                    ui.label(TextStyle::caption("Material: Equal"));
                }
            });
        });

    // === AI Statistics Panel (Bottom Left) - only show when playing vs AI ===
    if let GameMode::VsAI { .. } = params.ai_params.ai_config.mode {
        egui::Window::new("AI Statistics")
            .anchor(egui::Align2::LEFT_BOTTOM, [10.0, -10.0])
            .resizable(false)
            .collapsible(true)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.heading(TextStyle::heading("Engine Stats", TextSize::SM));
                    ui.add_space(5.0);

                    if params.ai_params.ai_stats.last_depth > 0 {
                        ui.label(TextStyle::caption(format!(
                            "Evaluation: {}",
                            format_score(params.ai_params.ai_stats.last_score)
                        )));
                        ui.label(TextStyle::caption(format!(
                            "Search depth: {}",
                            params.ai_params.ai_stats.last_depth
                        )));
                        ui.label(TextStyle::caption(format!(
                            "Nodes: {}",
                            format_nodes(params.ai_params.ai_stats.last_nodes)
                        )));

                        if params.ai_params.ai_stats.thinking_time > 0.0 {
                            ui.label(TextStyle::caption(format!(
                                "Thinking time: {:.2}s",
                                params.ai_params.ai_stats.thinking_time
                            )));
                        }

                        if params.ai_params.pending_ai.is_some() {
                            ui.colored_label(
                                egui::Color32::from_rgb(100, 150, 255),
                                "Calculating...",
                            );
                        }
                    } else {
                        ui.label(TextStyle::caption("Waiting for first move..."));
                    }
                });
            });
    }

    Ok(())
}

/// Format engine score for display
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
