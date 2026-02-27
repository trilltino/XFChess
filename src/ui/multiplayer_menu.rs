use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::{egui, EguiContexts};
use serde::{Deserialize, Serialize};

use crate::core::GameState;
use crate::multiplayer::BraidNetworkState;

/// System parameter to handle multiplayer menu interactions
#[derive(SystemParam)]
pub struct MultiplayerMenu<'w> {
    game_states: ResMut<'w, NextState<GameState>>,
    braid_network: ResMut<'w, BraidNetworkState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultiplayerMode {
    AutomaticMatchmaking,
    AiOpponent,
    GossipMatchmaking,
    BraidUriInput,
}

#[derive(Resource)]
pub struct MultiplayerMenuState {
    pub selected_mode: MultiplayerMode,
    pub is_searching: bool,
    pub selected_wager: f64,
    pub status_text: String,
    pub error_message: Option<String>,
    pub braid_uri_input: String,
}

impl Default for MultiplayerMenuState {
    fn default() -> Self {
        Self {
            selected_mode: MultiplayerMode::AutomaticMatchmaking,
            is_searching: false,
            selected_wager: 0.1,
            status_text: String::new(),
            error_message: None,
            braid_uri_input: String::new(),
        }
    }
}

pub fn multiplayer_menu_system(
    mut contexts: EguiContexts,
    mut menu_state: ResMut<MultiplayerMenuState>,
    mut multiplayer_menu: MultiplayerMenu,
    // Add Braid/Iroh resources here when implemented
) {
    let Some(ctx) = contexts.ctx_mut().ok() else {
        return;
    };

    egui::Window::new("Multiplayer Menu")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading("Multiplayer Options");

            // Display current node ID if available
            if let Some(node_id) = &multiplayer_menu.braid_network.node_id {
                let node_id_str = bs58::encode(node_id).into_string();
                ui.label(format!(
                    "Your Node ID: {}",
                    &node_id_str[..16.min(node_id_str.len())]
                ));
            }

            // Radio buttons for different multiplayer modes
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut menu_state.selected_mode,
                    MultiplayerMode::GossipMatchmaking,
                    "Gossip Matchmaking",
                );
                ui.selectable_value(
                    &mut menu_state.selected_mode,
                    MultiplayerMode::BraidUriInput,
                    "Connect to Braid URI",
                );
                ui.selectable_value(
                    &mut menu_state.selected_mode,
                    MultiplayerMode::AiOpponent,
                    "Play vs AI",
                );
            });

            match menu_state.selected_mode {
                MultiplayerMode::AutomaticMatchmaking => {
                    ui.separator();
                    ui.label("Automatic matchmaking via Braid network");

                    if ui.button("Find Match").clicked() {
                        menu_state.is_searching = true;
                        menu_state.status_text = "Finding match...".to_string();
                    }
                }
                MultiplayerMode::GossipMatchmaking => {
                    ui.separator();
                    ui.label("Find opponents via decentralized gossip protocol");

                    if ui.button("Search for Opponents").clicked() {
                        // This would initiate gossip-based matchmaking
                        initiate_gossip_matchmaking(&mut multiplayer_menu, &mut menu_state);
                    }

                    // Show discovered peers
                    if !multiplayer_menu.braid_network.discovered_peers.is_empty() {
                        ui.separator();
                        ui.label("Discovered Peers:");
                        for peer in &multiplayer_menu.braid_network.discovered_peers {
                            ui.label(format!("- {}: {}", &peer.node_id[..8], peer.wallet_address));
                        }
                    }
                }
                MultiplayerMode::BraidUriInput => {
                    ui.separator();
                    ui.label("Enter Braid URI to connect to a specific opponent:");

                    ui.text_edit_singleline(&mut menu_state.braid_uri_input);

                    if ui.button("Connect").clicked() {
                        // This would parse the Braid URI and establish connection
                        connect_via_braid_uri(&mut multiplayer_menu, &mut menu_state);
                    }
                }
                MultiplayerMode::AiOpponent => {
                    ui.separator();
                    ui.label("Play against an AI opponent");

                    if ui.button("Start Game vs AI").clicked() {
                        // Transition to game state with AI
                        multiplayer_menu.game_states.set(GameState::InGame);
                    }
                }
            }

            // Show error messages if any
            if let Some(error) = &menu_state.error_message {
                ui.separator();
                ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
            }

            // Show connecting status
            if menu_state.is_searching {
                ui.separator();
                ui.label("Searching for opponents...");
            }

            // Back button
            if ui.button("Back").clicked() {
                multiplayer_menu.game_states.set(GameState::MainMenu);
            }
        });
}

/// Initiates gossip-based matchmaking to find opponents
fn initiate_gossip_matchmaking(_menu: &mut MultiplayerMenu, state: &mut MultiplayerMenuState) {
    state.is_searching = true;
    state.status_text = "Searching for opponents via gossip protocol...".to_string();
    state.error_message = None;
    // TODO: Implement actual gossip matchmaking logic
}

/// Connects to a specific opponent via Braid URI
fn connect_via_braid_uri(menu: &mut MultiplayerMenu, state: &mut MultiplayerMenuState) {
    if state.braid_uri_input.is_empty() {
        state.error_message = Some("Please enter a Braid URI".to_string());
        return;
    }

    state.is_searching = true;
    state.status_text = format!(
        "Connecting to {}...",
        &state.braid_uri_input[..16.min(state.braid_uri_input.len())]
    );
    state.error_message = None;

    // Transition to in-game state for now (TODO: actual connection logic)
    menu.game_states.set(GameState::InGame);
}
