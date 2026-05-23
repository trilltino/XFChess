use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::{egui, EguiContexts};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::core::GameState;

#[cfg(feature = "solana")]
use crate::multiplayer::solana::addon::SolanaWallet;

#[cfg(not(feature = "solana"))]
#[derive(bevy::prelude::Resource)]
pub struct SolanaWallet {
    pub pubkey: Option<String>,
}

// use crate::multiplayer::BraidNetworkState; // Temporarily disabled

#[derive(Debug, Clone, Deserialize)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub max_players: u16,
    pub registered: usize,
    pub status: String,
}

#[derive(Resource, Clone)]
pub struct TournamentsFetchChannel {
    pub receiver: Arc<Mutex<std::sync::mpsc::Receiver<Vec<TournamentSummary>>>>,
    pub sender: std::sync::mpsc::SyncSender<Vec<TournamentSummary>>,
}

pub struct MultiplayerMenuPlugin;

impl Plugin for MultiplayerMenuPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<TournamentSummary>>(4);
        app.insert_resource(TournamentsFetchChannel {
            receiver: Arc::new(Mutex::new(rx)),
            sender: tx,
        })
        .init_resource::<MultiplayerMenuState>()
        .add_systems(Update, process_tournament_fetch_system)
        .add_systems(
            bevy_egui::EguiPrimaryContextPass,
            multiplayer_menu_system.run_if(in_state(GameState::MultiplayerMenu)),
        );
    }
}

/// System parameter to handle multiplayer menu interactions
#[derive(SystemParam)]
pub struct MultiplayerMenu<'w> {
    game_states: ResMut<'w, NextState<GameState>>,
    // braid_network: ResMut<'w, BraidNetworkState>, // Temporarily disabled
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultiplayerMode {
    AutomaticMatchmaking,
    AiOpponent,
    GossipMatchmaking,
    BraidUriInput,
    MyTournaments,
}

#[derive(Resource)]
pub struct MultiplayerMenuState {
    pub selected_mode: MultiplayerMode,
    pub is_searching: bool,
    pub status_text: String,
    pub error_message: Option<String>,
    pub braid_uri_input: String,
    pub my_tournaments: Option<Vec<TournamentSummary>>,
    pub loading_tournaments: bool,
    pub leaving_tournament: Option<u64>,
}

impl Default for MultiplayerMenuState {
    fn default() -> Self {
        Self {
            selected_mode: MultiplayerMode::AutomaticMatchmaking,
            is_searching: false,
            status_text: String::new(),
            error_message: None,
            braid_uri_input: String::new(),
            my_tournaments: None,
            loading_tournaments: false,
            leaving_tournament: None,
        }
    }
}

pub fn multiplayer_menu_system(
    mut contexts: EguiContexts,
    mut menu_state: ResMut<MultiplayerMenuState>,
    mut multiplayer_menu: MultiplayerMenu,
    wallet_opt: Option<Res<SolanaWallet>>,
    channel_opt: Option<Res<TournamentsFetchChannel>>,
) {
    let Some(ctx) = contexts.ctx_mut().ok() else {
        return;
    };

    egui::Window::new("Multiplayer Menu")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading("Multiplayer Options");

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
                ui.selectable_value(
                    &mut menu_state.selected_mode,
                    MultiplayerMode::MyTournaments,
                    "My Tournaments",
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
                MultiplayerMode::MyTournaments => {
                    ui.separator();
                    ui.label("Tournaments you have registered for:");
                    if menu_state.loading_tournaments {
                        ui.label("Loading tournaments...");
                    } else if let Some(tourneys) = &menu_state.my_tournaments {
                        if tourneys.is_empty() {
                            ui.label("You have not joined any tournaments.");
                        } else {
                            let tourneys_cloned = tourneys.clone();
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for t in &tourneys_cloned {
                                    ui.group(|ui| {
                                        ui.heading(&t.name);
                                        ui.label(format!("Players: {}/{}", t.registered, t.max_players));
                                        ui.label(format!("Status: {}", t.status));
                                        if t.status == "Registration" {
                                            ui.horizontal(|ui| {
                                                if ui.add_enabled(false, egui::Button::new(format!("Waiting for Players ({}/{})", t.registered, t.max_players))).clicked() {}
                                                
                                                let is_leaving = menu_state.leaving_tournament == Some(t.tournament_id);
                                                if ui.add_enabled(!is_leaving, egui::Button::new("Leave")).clicked() {
                                                    menu_state.leaving_tournament = Some(t.tournament_id);
                                                    let t_id = t.tournament_id;
                                                    let wallet_ready = wallet_opt.as_ref().map(|w| w.pubkey.is_some()).unwrap_or(false);
                                                    
                                                    if wallet_ready {
                                                        let Some(wallet) = wallet_opt.as_ref() else {
                                                            warn!("[MENU] Wallet not available for leave");
                                                            return;
                                                        };
                                                        let Some(pubkey) = wallet.pubkey.as_ref() else {
                                                            warn!("[MENU] Pubkey not available");
                                                            return;
                                                        };
                                                        let pubkey_str = pubkey.clone();
                                                        let Some(channel) = channel_opt.as_ref() else {
                                                            warn!("[MENU] Channel not available");
                                                            return;
                                                        };
                                                        let sender = channel.sender.clone(); // Reuse sender for refresh
                                                        
                                                        #[cfg(feature = "solana")]
                                                        std::thread::spawn(move || {
                                                            let vps_url = std::env::var("SIGNING_SERVICE_URL")
                                                                .or_else(|_| std::env::var("BACKEND_URL"))
                                                                .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string());
                                                            
                                                            // 1. Build Leave Transaction
                                                            let build_url = format!("{}/tournament/{}/build-leave-tx", vps_url, t_id);
                                                            let client = reqwest::blocking::Client::new();
                                                            let resp = client.post(&build_url)
                                                                .json(&serde_json::json!({ "player": pubkey_str }))
                                                                .send();
                                                                
                                                            if let Ok(r) = resp {
                                                                if r.status().is_success() {
                                                                    if let Ok(data) = r.json::<serde_json::Value>() {
                                                                        if let Some(tx_b64) = data["transaction"].as_str() {
                                                                            // 2. Sign and Send via Tauri bridge
                                                                            let sign_res = crate::multiplayer::solana::tauri_signer::sign_and_send_b64_via_tauri(crate::multiplayer::solana::integration::state::DEVNET_RPC_URL, tx_b64);
                                                                            
                                                                            if sign_res.is_ok() {
                                                                                // 3. Confirm with backend
                                                                                let leave_url = format!("{}/tournament/{}/leave", vps_url, t_id);
                                                                                let _ = client.post(&leave_url)
                                                                                    .json(&serde_json::json!({ "player": pubkey_str }))
                                                                                    .send();
                                                                                    
                                                                                // 4. Refresh tournaments list
                                                                                let refresh_url = format!("{}/tournament/my?player={}", vps_url, pubkey_str);
                                                                                if let Ok(refresh_resp) = reqwest::blocking::get(&refresh_url) {
                                                                                    if let Ok(new_data) = refresh_resp.json::<Vec<TournamentSummary>>() {
                                                                                        let _ = sender.try_send(new_data);
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        });
                                                        
                                                        #[cfg(not(feature = "solana"))]
                                                        {
                                                            let _ = t_id;
                                                            let _ = pubkey_str;
                                                            let _ = sender;
                                                            warn!("Solana feature is disabled, cannot leave tournament.");
                                                        }
                                                    }
                                                }
                                            });
                                        } else if t.status == "Active" {
                                            if ui.button("Join Match").clicked() {
                                                menu_state.status_text = format!("Joining match in {}...", t.name);
                                                menu_state.is_searching = true;
                                                // Trigger match connection logic
                                            }
                                        }
                                    });
                                }
                            });
                        }
                    } else {
                        if ui.button("Fetch My Tournaments").clicked() {
                            if let Some(wallet) = &wallet_opt {
                                if let Some(pubkey) = &wallet.pubkey {
                                    if let Some(channel) = &channel_opt {
                                        menu_state.loading_tournaments = true;
                                        let pubkey_str = pubkey.to_string();
                                        let tx = channel.sender.clone();
                                        
                                        std::thread::spawn(move || {
                                            let vps_url = std::env::var("SIGNING_SERVICE_URL")
                                                .or_else(|_| std::env::var("BACKEND_URL"))
                                                .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string());
                                            let url = format!("{}/tournament/my?player={}", vps_url, pubkey_str);
                                            match reqwest::blocking::get(&url) {
                                                Ok(resp) if resp.status().is_success() => {
                                                    if let Ok(data) = resp.json::<Vec<TournamentSummary>>() {
                                                        let _ = tx.try_send(data);
                                                    }
                                                }
                                                _ => {}
                                            }
                                        });
                                    } else {
                                        menu_state.error_message = Some("Fetch channel missing".to_string());
                                    }
                                } else {
                                    menu_state.error_message = Some("Wallet not connected".to_string());
                                }
                            } else {
                                menu_state.error_message = Some("Wallet resource missing".to_string());
                            }
                        }
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

    menu.game_states.set(GameState::InGame);
}

pub fn process_tournament_fetch_system(
    mut menu_state: ResMut<MultiplayerMenuState>,
    channel_opt: Option<Res<TournamentsFetchChannel>>,
) {
    if let Some(channel) = channel_opt {
        if let Ok(rx) = channel.receiver.lock() {
            while let Ok(tourneys) = rx.try_recv() {
                menu_state.my_tournaments = Some(tourneys);
                menu_state.loading_tournaments = false;
                menu_state.leaving_tournament = None;
            }
        }
    }
}
