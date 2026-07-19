//! Spectator mode UI for watching live games
//!
//! Provides a read-only view of ongoing games using egui (following project patterns)

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

/// Resource tracking spectator mode state
#[derive(Resource, Default)]
pub struct SpectatorMode {
    /// Whether spectator mode is active
    pub active: bool,
    /// Currently spectated game ID
    pub game_id: String,
    /// Last received FEN position
    pub current_fen: String,
    /// Move history
    pub moves: Vec<String>,
    /// White player info
    pub white_player: Option<PlayerInfo>,
    /// Black player info
    pub black_player: Option<PlayerInfo>,
    /// Connection status
    pub connected: bool,
    /// Error message if any
    pub error: Option<String>,
}

/// Player information for display
#[derive(Clone, Debug)]
pub struct PlayerInfo {
    pub username: String,
    pub rating: u32,
    pub country: String,
}

/// Plugin for spectator mode UI
pub struct SpectatorModePlugin;

impl Plugin for SpectatorModePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorMode>()
            // Must run in EguiPrimaryContextPass — EguiContexts is only valid there.
            // The run condition avoids touching the egui context every frame when
            // spectator mode is inactive, which previously blocked the main menu UI.
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                spectator_ui_system.run_if(|s: Res<SpectatorMode>| s.active),
            );
    }
}

/// Main spectator UI system
///
/// Runs only in [`bevy_egui::EguiPrimaryContextPass`] and only when
/// [`SpectatorMode::active`] is true, so it never touches the egui context
/// during normal gameplay or menu rendering.
fn spectator_ui_system(
    mut contexts: EguiContexts,
    spectator: Res<SpectatorMode>,
    state: Res<State<crate::core::GameState>>,
    game_mode: Res<crate::core::states::GameMode>,
) {
    // HIDE ASCII window if we are in the "Full" spectator mode (InGame board)
    if *state.get() == crate::core::GameState::InGame
        && *game_mode == crate::core::states::GameMode::Spectator
    {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    egui::Window::new("Spectator Mode")
        .default_size([800.0, 600.0])
        .resizable(true)
        .show(ctx, |ui| {
            // Header with game ID
            ui.horizontal(|ui| {
                ui.heading(format!("Game: {}", spectator.game_id));

                if spectator.connected {
                    ui.colored_label(egui::Color32::GREEN, "Live");
                } else {
                    ui.colored_label(egui::Color32::RED, "Disconnected");
                }
            });

            ui.separator();

            // Player info panel
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading("White");
                    if let Some(ref white) = spectator.white_player {
                        ui.label(&white.username);
                        ui.label(format!("Rating: {}", white.rating));
                    } else {
                        ui.label("Waiting...");
                    }
                });

                ui.separator();
                ui.vertical_centered(|ui| ui.heading("VS"));
                ui.separator();

                ui.vertical(|ui| {
                    ui.heading("Black");
                    if let Some(ref black) = spectator.black_player {
                        ui.label(&black.username);
                        ui.label(format!("Rating: {}", black.rating));
                    } else {
                        ui.label("Waiting...");
                    }
                });
            });

            ui.separator();

            // Chess board view (placeholder - shows FEN)
            ui.group(|ui| {
                ui.heading("Board Position");
                ui.monospace(&spectator.current_fen);
                if !spectator.current_fen.is_empty() {
                    ui.separator();
                    render_simple_board(ui, &spectator.current_fen);
                }
            });

            ui.separator();

            // Move history
            ui.group(|ui| {
                ui.heading("Move History");
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for (i, mv) in spectator.moves.iter().enumerate() {
                            let move_num = i / 2 + 1;
                            let is_white = i % 2 == 0;
                            let prefix = if is_white {
                                format!("{}", move_num)
                            } else {
                                String::new()
                            };
                            ui.label(format!("{} {}", prefix, mv));
                        }
                    });
            });

            // Error display
            if let Some(ref error) = spectator.error {
                ui.separator();
                ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
            }
        });
}

/// Render a simple ASCII chess board from FEN
fn render_simple_board(ui: &mut egui::Ui, fen: &str) {
    // Parse FEN and render basic board
    let board_part = fen.split_whitespace().next().unwrap_or("");

    ui.monospace("+-------------------------------+");

    for rank in (0..8).rev() {
        let mut row_str = String::from("¦");

        for file in 0..8 {
            // Find piece at this position
            let square = get_square_from_fen(board_part, file, rank);
            row_str.push_str(&format!(" {} ¦", square));
        }

        ui.monospace(&row_str);

        if rank > 0 {
            ui.monospace("+---+---+---+---+---+---+---+---¦");
        }
    }

    ui.monospace("+-------------------------------+");
    ui.monospace("  a   b   c   d   e   f   g   h");
}

/// Get piece symbol at a specific square from FEN
fn get_square_from_fen(fen_board: &str, file: usize, rank: usize) -> String {
    let ranks: Vec<&str> = fen_board.split('/').collect();
    if rank >= ranks.len() {
        return " ".to_string();
    }

    let rank_str = ranks[rank];
    let mut current_file = 0;

    for c in rank_str.chars() {
        if c.is_ascii_digit() {
            let empty_squares = c.to_digit(10).unwrap_or(0) as usize;
            if current_file + empty_squares > file {
                return " ".to_string();
            }
            current_file += empty_squares;
        } else {
            if current_file == file {
                return piece_symbol(c);
            }
            current_file += 1;
        }
    }

    " ".to_string()
}

/// Convert FEN piece character to Unicode chess symbol
fn piece_symbol(c: char) -> String {
    match c {
        'P' => "".to_string(),
        'N' => "".to_string(),
        'B' => "".to_string(),
        'R' => "".to_string(),
        'Q' => "".to_string(),
        'K' => "".to_string(),
        'p' => "".to_string(),
        'n' => "".to_string(),
        'b' => "".to_string(),
        'r' => "".to_string(),
        'q' => "".to_string(),
        'k' => "".to_string(),
        _ => " ".to_string(),
    }
}

/// System to add spectator menu option to main menu.
///
/// Must be scheduled in [`bevy_egui::EguiPrimaryContextPass`] by the caller.
pub fn spectator_menu_ui(
    mut contexts: EguiContexts,
    mut spectator: ResMut<SpectatorMode>,
    mut next_state: ResMut<NextState<crate::core::GameState>>,
    mut game_mode: ResMut<crate::core::states::GameMode>,
    braid_network: Res<crate::multiplayer::OnlineNetworkState>,
) {
    // Show join dialog only when spectator mode is not yet active
    if spectator.active {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    egui::Window::new("Spectate Game")
        .collapsible(false)
        .resizable(false)
        .default_width(320.0)
        .show(ctx, |ui| {
            // Collect unique active games from discovered peers
            let mut active_games = std::collections::HashSet::new();
            for peer in &braid_network.discovered_peers {
                if let Some(game_id) = peer.connected_game {
                    active_games.insert(game_id);
                }
            }

            if active_games.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Looking for active games...").italics().color(egui::Color32::from_rgb(150, 150, 150)));
                });
            } else {
                ui.label(egui::RichText::new(format!("Discovered {} Live Games:", active_games.len())).strong());
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(5.0);

                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for game_id in active_games {
                        ui.horizontal(|ui| {
                            ui.label(format!("Game #{}", game_id));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("WATCH").clicked() {
                                    spectator.game_id = game_id.to_string();
                                    spectator.active = true;
                                    spectator.connected = true;
                                    spectator.current_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string();
                                    
                                    // Transition to Full Spectator Mode
                                    {
                                        let next_mode = crate::core::states::GameMode::Spectator;
                                        if *game_mode != next_mode {
                                            *game_mode = next_mode;
                                        }
                                    }
                                    next_state.set(crate::core::GameState::InGame);
                                }
                            });
                        });
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                    }
                });
            }

            ui.add_space(10.0);
            if ui.button("Close").clicked() {
                // Return to main menu (by convention, closing the window)
            }
        });
}
