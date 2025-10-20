//! In-game UI for captured pieces, game status, and AI thinking indicator

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::core::GameState;
use crate::rendering::pieces::{PieceType, PieceColor};
use crate::game::components::GamePhase;
use crate::game::ai::GameMode;
use crate::game::plugin::{GameStateParams, AIParams};

/// System to display captured pieces and game status during gameplay
pub fn game_status_ui(
    mut contexts: EguiContexts,
    game_state: GameStateParams,
    ai_params: AIParams,
    mut next_state: ResMut<NextState<GameState>>,
) -> Result<(), bevy::ecs::query::QuerySingleError> {
    let ctx = contexts.ctx_mut()?;

    // Captured Pieces Panel (Right side)
    egui::Window::new("Captured Pieces")
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // White's captures (black pieces taken)
                ui.heading("White Captured:");
                ui.horizontal_wrapped(|ui| {
                    if game_state.captured.white_captured.is_empty() {
                        ui.label("None");
                    } else {
                        for piece in &game_state.captured.white_captured {
                            ui.label(piece_unicode(*piece, PieceColor::Black));
                        }
                    }
                });

                ui.add_space(10.0);

                // Black's captures (white pieces taken)
                ui.heading("Black Captured:");
                ui.horizontal_wrapped(|ui| {
                    if game_state.captured.black_captured.is_empty() {
                        ui.label("None");
                    } else {
                        for piece in &game_state.captured.black_captured {
                            ui.label(piece_unicode(*piece, PieceColor::White));
                        }
                    }
                });

                ui.add_space(10.0);

                // Material advantage
                let advantage = game_state.captured.material_advantage();
                if advantage > 0 {
                    ui.label(format!("White: +{}", advantage));
                } else if advantage < 0 {
                    ui.label(format!("Black: +{}", -advantage));
                } else {
                    ui.label("Material: Equal");
                }
            });
        });

    // Game Status Panel (Top center)
    egui::Window::new("Game Status")
        .anchor(egui::Align2::CENTER_TOP, [0.0, 10.0])
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Current turn
                if !game_state.game_over.is_game_over() {
                    match game_state.current_turn.color {
                        PieceColor::White => ui.heading("White's Turn"),
                        PieceColor::Black => ui.heading("Black's Turn"),
                    };
                }

                ui.add_space(5.0);

                // Game over notification or game phase status
                if game_state.game_over.is_game_over() {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 200, 0),
                        egui::RichText::new(game_state.game_over.message()).size(20.0)
                    );

                    ui.add_space(10.0);

                    if ui.button(egui::RichText::new("🔄 New Game").size(18.0)).clicked() {
                        next_state.set(GameState::LaunchMenu);
                    }

                    ui.add_space(5.0);
                } else {
                    // Game phase status
                    match game_state.game_phase.0 {
                        GamePhase::Check => {
                            ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "⚠️ CHECK!");
                        }
                        GamePhase::Checkmate => {
                            ui.colored_label(egui::Color32::from_rgb(255, 0, 0), "🏁 CHECKMATE!");
                        }
                        GamePhase::Stalemate => {
                            ui.colored_label(egui::Color32::from_rgb(200, 200, 0), "🤝 STALEMATE");
                        }
                        GamePhase::Playing | GamePhase::Setup => {
                            // Show AI thinking indicator
                            if ai_params.pending_ai.is_some() {
                                ui.colored_label(egui::Color32::from_rgb(100, 150, 255), "🤔 AI is thinking...");
                            } else {
                                ui.label("▶️ Game in progress");
                            }
                        }
                    }

                    ui.add_space(5.0);
                }

                // Show game mode
                match ai_params.ai_config.mode {
                    GameMode::VsHuman => {
                        ui.label("Mode: Human vs Human");
                    }
                    GameMode::VsAI { ai_color } => {
                        let ai_str = match ai_color {
                            PieceColor::White => "White (AI)",
                            PieceColor::Black => "Black (AI)",
                        };
                        ui.label(format!("Mode: vs {} - {}", ai_str, ai_params.ai_config.difficulty.description()));
                    }
                }
            });
        });

    // AI Statistics Panel (Left side) - only show when playing vs AI
    if let GameMode::VsAI { .. } = ai_params.ai_config.mode {
        egui::Window::new("AI Statistics")
            .anchor(egui::Align2::LEFT_TOP, [10.0, 10.0])
            .resizable(false)
            .collapsible(true)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Engine Stats");
                    ui.add_space(5.0);

                    if ai_params.ai_stats.last_depth > 0 {
                        ui.label(format!("Evaluation: {}", format_score(ai_params.ai_stats.last_score)));
                        ui.label(format!("Search depth: {}", ai_params.ai_stats.last_depth));
                        ui.label(format!("Nodes: {}", format_nodes(ai_params.ai_stats.last_nodes)));

                        if ai_params.pending_ai.is_some() {
                            ui.colored_label(egui::Color32::from_rgb(100, 150, 255), "Calculating...");
                        }
                    } else {
                        ui.label("Waiting for first move...");
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

/// Convert piece type to Unicode chess character
fn piece_unicode(piece_type: PieceType, color: PieceColor) -> String {
    let symbol = match (color, piece_type) {
        (PieceColor::White, PieceType::King) => "♔",
        (PieceColor::White, PieceType::Queen) => "♕",
        (PieceColor::White, PieceType::Rook) => "♖",
        (PieceColor::White, PieceType::Bishop) => "♗",
        (PieceColor::White, PieceType::Knight) => "♘",
        (PieceColor::White, PieceType::Pawn) => "♙",
        (PieceColor::Black, PieceType::King) => "♚",
        (PieceColor::Black, PieceType::Queen) => "♛",
        (PieceColor::Black, PieceType::Rook) => "♜",
        (PieceColor::Black, PieceType::Bishop) => "♝",
        (PieceColor::Black, PieceType::Knight) => "♞",
        (PieceColor::Black, PieceType::Pawn) => "♟",
    };

    format!("{} ", symbol)
}
