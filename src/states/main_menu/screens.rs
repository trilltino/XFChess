//! Main menu screen and popup rendering helpers.
//!
//! This module contains the UI that was split out of `src/states/main_menu.rs`
//! so the main menu state stays focused on plugin setup and high-level flow.
//! The parent module calls into these helpers to render the Solana lobby,
//! braid lobby, tournament browser, host configuration, waiting screens, and
//! popups while keeping the shared state in `MainMenuUIContext`.

use super::*;
use crate::core::GameMode as CoreGameMode;
use crate::game::ai::GameMode as AIGameMode;
use crate::multiplayer::network::p2p::P2PConnectionStatus;
use crate::ui::styles::Layout;
use bevy_egui::egui;
use tracing::{error, info, warn};

#[cfg(feature = "solana")]
use crate::multiplayer::solana::lobby::{
    spawn_create_game, spawn_join_game, spawn_lookup_game, spawn_poll_opponent_joined,
    LobbyMode, LobbyStatus,
};

#[cfg(feature = "solana")]
use crate::multiplayer::solana::tauri_signer;

#[cfg(feature = "solana")]
pub(super) fn ui_solana_lobby(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    let Some(ref mut lobby) = ctx.solana_lobby else {
        ui.label("Solana lobby not available.");
        return;
    };

    lobby.cached_display_name = Some(ctx.player_identity.display_name().to_string());
    lobby.cached_node_id = ctx.network_state.as_ref().and_then(|ns| {
        ns.node_id.map(|id| bs58::encode(id.as_bytes()).into_string())
    });

    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        if ui.button("⬅ Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::ModeSelect);
            lobby.status = LobbyStatus::Idle;
        }

        Layout::section_space(ui);

        ui.label(
            egui::RichText::new("SOLANA WAGER LOBBY")
                .size(24.0)
                .color(egui::Color32::from_rgb(255, 150, 100))
                .strong(),
        );

        Layout::item_space(ui);

        // Wallet / balance header
        let balance = lobby.cached_balance;
        let wallet_ready = lobby.cached_keypair_bytes.is_some();
        if wallet_ready {
            ui.label(
                egui::RichText::new(format!("Wallet balance: {:.4} SOL", balance))
                    .color(egui::Color32::GOLD),
            );
        } else {
            ui.colored_label(egui::Color32::YELLOW, "Wallet not connected");
            if ui.button(" Connect Wallet").clicked() {
                tauri_signer::open_wallet_browser();
            }
        }

        Layout::item_space(ui);

        // Only show the create/join form while not in a post-transaction state.
        let in_post_state = matches!(
            lobby.status,
            LobbyStatus::WaitingForOpponent { .. }
                | LobbyStatus::OpponentJoined { .. }
                | LobbyStatus::Success(_)
        );

        if !in_post_state {
            // Tab switcher
            ui.horizontal(|ui| {
                if ui.selectable_label(lobby.mode == LobbyMode::Create, "Create Game").clicked() {
                    lobby.mode = LobbyMode::Create;
                    lobby.status = LobbyStatus::Idle;
                }
                if ui.selectable_label(lobby.mode == LobbyMode::Join, "Join Game").clicked() {
                    lobby.mode = LobbyMode::Join;
                    lobby.status = LobbyStatus::Idle;
                }
            });

            ui.separator();
            Layout::item_space(ui);

            let node_id_b58 = ctx.network_state.as_ref().and_then(|ns| {
                ns.node_id.map(|id| bs58::encode(id.as_bytes()).into_string())
            });
            match lobby.mode {
                LobbyMode::Create => render_create_tab(ui, lobby, &mut ctx.compliance, node_id_b58.as_deref()),
                LobbyMode::Join => render_join_tab(ui, lobby),
            }
        }

        Layout::item_space(ui);

        // Auto-transition: Success + Create mode → WaitingForOpponent + start poll.
        if let LobbyStatus::Success(game_id) = lobby.status {
            if lobby.mode == LobbyMode::Create && lobby.opponent_poll_rx.is_none() {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_poll_opponent_joined(lobby.cached_rpc_url.clone(), game_id, tx);
                lobby.opponent_poll_rx = Some(rx);
                lobby.status = LobbyStatus::WaitingForOpponent { game_id };
            }
        }

        // Post-action status UI (uses ctx for firing events).
        let status_snap = lobby.status.clone();
        let wager_lamports = lobby.wager_lamports();

        match status_snap {
            LobbyStatus::Idle => {}

            LobbyStatus::Pending => {
                ui.spinner();
                ui.label(
                    egui::RichText::new("⏳ Submitting transaction...")
                        .color(egui::Color32::from_rgb(200, 200, 50)),
                );
            }

            LobbyStatus::WaitingForOpponent { game_id: _ } => {
                ui.spinner();
                ui.label(
                    egui::RichText::new("⏳ Waiting for opponent to join...")
                        .color(egui::Color32::from_rgb(255, 200, 80)),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new("Your game room is ready. Opponent can join from the lobby list.")
                        .size(12.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );
                Layout::small_space(ui);
                if ui.small_button(" Cancel").clicked() {
                    lobby.status = LobbyStatus::Idle;
                    lobby.opponent_poll_rx = None;
                }
            }

            LobbyStatus::OpponentJoined { game_id } => {
                ui.label(
                    egui::RichText::new(" Opponent joined!")
                        .color(egui::Color32::from_rgb(100, 255, 100))
                        .strong(),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new(
                        "Click 'Host Game' to start the match.",
                    )
                    .size(12.0)
                    .color(egui::Color32::LIGHT_GRAY),
                );
                Layout::small_space(ui);
                if ui.button(" Host Game").clicked() {
                    ctx.ai_config.mode = AIGameMode::Multiplayer;
                    *ctx.core_mode = CoreGameMode::BraidMultiplayer;
                    if let Some(ref mut sync) = ctx.solana_sync {
                        sync.game_id = Some(game_id);
                        sync.wager_amount = wager_lamports;
                    }
                    if let Some(ref mut comp) = ctx.competitive {
                        comp.game_id = Some(game_id);
                        comp.wager_lamports = wager_lamports;
                        comp.active = true;
                    }
                    ctx.menu_state.set(crate::core::MenuState::Main);
                }
            }

            LobbyStatus::Fetched { .. } => {}

            LobbyStatus::Success(game_id) => {
                ui.label(
                    egui::RichText::new(" Game ready!")
                        .color(egui::Color32::from_rgb(100, 255, 100))
                        .strong(),
                );
                Layout::small_space(ui);
                if ui.button(" Start Game").clicked() {
                    ctx.ai_config.mode = AIGameMode::Multiplayer;
                    *ctx.core_mode = CoreGameMode::BraidMultiplayer;
                    if let Some(ref mut sync) = ctx.solana_sync {
                        sync.game_id = Some(game_id);
                        sync.wager_amount = wager_lamports;
                    }
                    if let Some(ref mut comp) = ctx.competitive {
                        comp.game_id = Some(game_id);
                        comp.wager_lamports = wager_lamports;
                        comp.active = true;
                    }
                    ctx.menu_state.set(crate::core::MenuState::Main);
                }
            }

            LobbyStatus::Error(msg) => {
                ui.colored_label(egui::Color32::RED, format!(" {}", msg));
                if ui.small_button("↩ Try Again").clicked() {
                    lobby.status = LobbyStatus::Idle;
                }
            }
        }
    });
}

/// Render spectator popup to view all games.
pub(super) fn render_spectator_popup(
    ctx: &egui::Context,
    competitive: &mut CompetitiveMenuState,
    cached_games: &[crate::multiplayer::network::p2p_vps::VpsGameListing],
) {
    let accent_color = egui::Color32::from_rgb(173, 92, 47);

    egui::Window::new("Spectator Mode")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::Vec2::new(520.0, 380.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .title_bar(false)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240),
            corner_radius: egui::CornerRadius::same(4),
            stroke: egui::Stroke::new(2.0, BEZEL_GREY),
            inner_margin: egui::Margin::same(16),
            ..egui::Frame::NONE
        })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Spectator Mode")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        competitive.show_spectator_popup = false;
                    }
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            if cached_games.is_empty() {
                ui.label(
                    egui::RichText::new("No games available to spectate at the moment.")
                        .size(14.0)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                );
                ui.label(
                    egui::RichText::new("Check back later for live games.")
                        .size(12.0)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 120))
                        .italics(),
                );
            } else {
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for game in cached_games {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    let host = game.username.as_deref().unwrap_or(&game.display_name);
                                    ui.label(egui::RichText::new(host).size(13.0).color(egui::Color32::WHITE).strong());
                                    let type_badge = match game.game_type.as_str() {
                                        "solana_wager" => " Wager",
                                        "tournament" => " Tournament",
                                        _ => " Free",
                                    };
                                    let stake_str = if game.stake_amount > 0.0 {
                                        format!("{} — {:.3} SOL", type_badge, game.stake_amount)
                                    } else {
                                        type_badge.to_string()
                                    };
                                    ui.label(egui::RichText::new(&stake_str).size(11.0).color(egui::Color32::LIGHT_GRAY));
                                    let tc_label = if game.base_time_seconds > 0 {
                                        let m = game.base_time_seconds / 60;
                                        let s = game.base_time_seconds % 60;
                                        if s == 0 { format!("⏱ {}+{}", m, game.increment_seconds) }
                                        else { format!("⏱ {}s+{}", game.base_time_seconds, game.increment_seconds) }
                                    } else { "⏱ ∞".to_string() };
                                    ui.label(egui::RichText::new(tc_label).size(10.0).color(egui::Color32::GRAY));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let game_id = game.game_id.clone();
                                    if ui.add_sized([60.0, 28.0], egui::Button::new(
                                        egui::RichText::new("Watch").size(11.0).color(egui::Color32::WHITE).strong()
                                    ).fill(egui::Color32::from_rgb(60, 100, 160)).corner_radius(4.0)).clicked() {
                                        info!("[SPECTATOR] Watch clicked for game {}", game_id);
                                    }
                                });
                            });
                        });
                        ui.add_space(4.0);
                    }
                });
            }

            ui.add_space(12.0);

            if ui.add_sized(
                [ui.available_width(), 36.0],
                egui::Button::new(egui::RichText::new("Close").size(14.0).color(egui::Color32::WHITE).strong())
                    .fill(accent_color)
                    .corner_radius(4.0),
            ).clicked() {
                competitive.show_spectator_popup = false;
            }
        });
}

/// Loading screen for the website-style menu.
pub(super) fn render_loading_screen_website(ctx: &egui::Context, ctx_menu: &mut MainMenuUIContext) {
    let screen_rect = ctx.input(|i| i.content_rect());

    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: egui::Color32::from_rgb(0, 0, 0),
            ..egui::Frame::NONE
        })
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(screen_rect.height() * 0.4);

                if ctx_menu.loading_progress.failed {
                    ui.heading(
                        egui::RichText::new("Asset Loading Failed")
                            .size(24.0)
                            .color(egui::Color32::from_rgb(220, 50, 50)),
                    );

                    if let Some(ref error_msg) = ctx_menu.loading_progress.error_message {
                        ui.label(
                            egui::RichText::new(error_msg)
                                .size(14.0)
                                .color(egui::Color32::from_rgb(220, 150, 150)),
                        );
                    }

                    if ui.button("Continue Anyway").clicked() {
                        ctx_menu.loading_progress.complete = true;
                        ctx_menu.game_assets.loaded = true;
                    }
                } else {
                    ui.heading(
                        egui::RichText::new("Loading...")
                            .size(24.0)
                            .color(egui::Color32::WHITE),
                    );

                    let progress_bar = egui::ProgressBar::new(ctx_menu.loading_progress.progress)
                        .desired_width(300.0)
                        .show_percentage()
                        .animate(true);

                    ui.add(progress_bar);
                }
            });
        });
}

/// Render join lobby popup.
pub(super) fn render_join_lobby_popup(
    ctx: &egui::Context,
    ctx_menu: &mut MainMenuUIContext,
) {
    egui::Window::new("Join a Lobby")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::Vec2::new(500.0, 320.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .title_bar(false)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240),
            corner_radius: egui::CornerRadius::same(4),
            stroke: egui::Stroke::new(2.0, BEZEL_GREY),
            inner_margin: egui::Margin::same(16),
            ..egui::Frame::NONE
        })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Join a Lobby")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("X").clicked() {
                        ctx_menu.competitive_menu.show_join_popup = false;
                    }
                });
            });

            ui.add_space(12.0);

            if let Some(vps_state) = ctx_menu.p2p_vps_state.as_ref() {
                if !vps_state.cached_games.is_empty() {
                    for listing in &vps_state.cached_games {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&listing.display_name)
                                        .size(14.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                );
                                let prize = if listing.stake_amount > 0.0 {
                                    format!("{:.3} SOL", listing.stake_amount)
                                } else {
                                    "Free".to_string()
                                };
                                ui.label(
                                    egui::RichText::new(prize)
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(150, 200, 150)),
                                );
                                let type_label = if listing.stake_amount > 0.0 {
                                    (" Wagered", egui::Color32::from_rgb(230, 57, 70))
                                } else {
                                    (" Free", egui::Color32::from_rgb(100, 200, 100))
                                };
                                ui.label(
                                    egui::RichText::new(type_label.0)
                                        .size(10.0)
                                        .color(type_label.1)
                                        .strong(),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .add_sized(
                                        [80.0, 28.0],
                                        egui::Button::new(
                                            egui::RichText::new("Join")
                                                .size(12.0)
                                                .color(egui::Color32::WHITE)
                                                .strong(),
                                        )
                                        .fill(egui::Color32::from_rgb(100, 200, 100))
                                        .corner_radius(4.0)
                                        .stroke(egui::Stroke::new(
                                            1.0,
                                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                                        )),
                                    )
                                    .clicked()
                                {
                                    info!("[MENU] Joining lobby: {}", listing.game_id);
                                    let game_id = listing.game_id.clone();
                                    let stake_amount = listing.stake_amount;
                                    let local_node_id = ctx_menu.network_state.as_ref()
                                        .and_then(|ns| ns.node_id.as_ref().map(|id| bs58::encode(id.as_bytes()).into_string()))
                                        .unwrap_or_else(|| "unknown".to_string());
                                    let response_tx = ctx_menu.p2p_vps_state.as_ref().map(|vps| vps.response_tx.clone());

                                    if let Some(tx) = response_tx {
                                        bevy::tasks::IoTaskPool::get().spawn(async move {
                                            match crate::multiplayer::network::vps::p2p_join_game(game_id.clone(), &local_node_id) {
                                                Ok(Some(host_id)) => {
                                                    let _ = tx.send(crate::multiplayer::network::p2p_vps::VpsResponse::JoinResult {
                                                        game_id,
                                                        host_node_id: Some(host_id),
                                                        stake_amount,
                                                    });
                                                }
                                                Ok(None) => warn!("[MENU] Join rejected by VPS for {}", game_id),
                                                Err(e) => error!("[MENU] Join error for {}: {}", game_id, e),
                                            }
                                        }).detach();
                                    } else {
                                        warn!("[MENU] Join requested but VPS relay state is unavailable");
                                    }

                                    ctx_menu.competitive_menu.show_join_popup = false;
                                }
                            });
                        });
                        ui.add_space(8.0);
                    }
                } else {
                    ui.label(
                        egui::RichText::new("No open lobbies available.")
                            .size(14.0)
                            .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                    );
                }
            } else {
                ui.label(
                    egui::RichText::new("No lobby data available.")
                        .size(14.0)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                );
            }
        });
}

#[cfg(feature = "solana")]
fn render_create_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    compliance: &mut crate::ui::compliance_modal::ComplianceState,
    node_id: Option<&str>,
) {
    let balance = lobby.cached_balance;
    let wallet_connected = lobby.cached_keypair_bytes.is_some();
    let max_wager = if wallet_connected {
        ((balance - 0.002) as f32).max(0.0)
    } else {
        0.0
    };

    // If wallet is not connected force wager to 0 (free game only).
    if !wallet_connected {
        lobby.wager_sol = 0.0;
    }

    ui.label(egui::RichText::new("Wager amount (SOL)").size(14.0));

    if wallet_connected {
        ui.add(
            egui::Slider::new(&mut lobby.wager_sol, 0.0..=max_wager.max(0.001))
                .step_by(0.001)
                .fixed_decimals(3),
        );
        // Clamp after slider interaction
        lobby.wager_sol = lobby.wager_sol.clamp(0.0, max_wager);
    } else {
        // Greyed-out disabled slider at 0
        let mut zero: f32 = 0.0;
        ui.add_enabled(
            false,
            egui::Slider::new(&mut zero, 0.0..=1.0).fixed_decimals(3),
        );
        ui.label(
            egui::RichText::new("(Connect wallet to add a wager)")
                .size(11.0)
                .color(egui::Color32::from_rgb(160, 130, 80))
                .italics(),
        );
    }

    let label_text = if lobby.wager_sol == 0.0 {
        "Free casual game — no SOL at stake".to_string()
    } else {
        format!("Escrow: {:.4} SOL  |  Pot: {:.4} SOL",
            lobby.wager_sol, lobby.wager_sol * 2.0)
    };
    ui.label(
        egui::RichText::new(label_text)
            .color(egui::Color32::LIGHT_GRAY)
            .size(12.0),
    );

    Layout::small_space(ui);

    let is_free_game = lobby.wager_sol == 0.0;
    let can_create = !matches!(lobby.status, LobbyStatus::Pending)
        && (is_free_game
            || (wallet_connected
                && lobby.wager_sol > 0.0
                && (lobby.wager_sol as f64) <= balance - 0.002))
        && !matches!(lobby.status, LobbyStatus::Pending);

    let is_devnet = lobby.cached_rpc_url.contains("devnet");
    
    if ui.add_sized([ui.available_width(), 40.0], egui::Button::new(
        egui::RichText::new(if is_free_game { " Host Free Game" } else { " Create Wagered Game" })
            .size(16.0)
            .strong()
    ).fill(if can_create { egui::Color32::from_rgb(40, 100, 40) } else { egui::Color32::from_rgb(40, 40, 40) }))
    .clicked() && can_create {
        if is_free_game {
            // Free game: announce to VPS relay with stake 0.0, then go straight into game.
            // No Solana transaction needed.
            let game_id = format!("free_{}", rand::random::<u32>());
            let display_name = lobby
                .cached_keypair_bytes
                .as_ref()
                .map(|b| {
                    // Show first 6 chars of hex as a display name.
                    let hex: String = b.iter().take(3).map(|x| format!("{:02x}", x)).collect();
                    format!("Player {}", &hex)
                })
                .unwrap_or_else(|| "Anonymous".to_string());

            let host_node_id = node_id.unwrap_or("unknown_node_id");
            match crate::multiplayer::vps_client::p2p_announce_game(
                game_id.clone(),
                host_node_id,
                &display_name,
                0.0,
                "P2P",
                300,
                0,
                Some(display_name.clone()),
                None,
                None,
            ) {
                Ok(()) => {
                    info!("[LOBBY] Free game announced (id={}). Waiting for opponent via VPS.", game_id);
                    lobby.status = LobbyStatus::WaitingForOpponent { game_id: 0 };
                }
                Err(e) => {
                    warn!("[LOBBY] VPS announce failed ({}). Starting locally anyway.", e);
                    lobby.status = LobbyStatus::WaitingForOpponent { game_id: 0 };
                }
            }
        } else {
            // Wagered game: enforce CARF compliance on mainnet, then spawn Solana TX.
            if !is_devnet && compliance.status != crate::ui::compliance_modal::SubmissionStatus::Success {
                compliance.show = true;
                if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                    compliance.pubkey = Some(wallet_pubkey.to_string());
                }
            } else if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_create_game(
                    lobby.cached_rpc_url.clone(),
                    wallet_pubkey,
                    lobby.wager_lamports(),
                    tx,
                );
                lobby.tx_rx = Some(rx);
                lobby.status = LobbyStatus::Pending;
                info!("[SOLANA_LOBBY] Creating wagered game ({} SOL)", lobby.wager_sol);
                let wager_sol = lobby.wager_sol as f64;
                let host_node_id_owned = node_id.unwrap_or("unknown_node_id").to_string();
                let disp = lobby.cached_keypair_bytes.as_ref()
                    .map(|b| { let hex: String = b.iter().take(3).map(|x| format!("{:02x}", x)).collect(); format!("Player {}", &hex) })
                    .unwrap_or_else(|| "Anonymous".to_string());
                let _ = crate::multiplayer::vps_client::p2p_announce_game(
                    format!("solana_{}", wallet_pubkey),
                    &host_node_id_owned,
                    &disp,
                    wager_sol,
                    "solana_wager",
                    300,
                    0,
                    Some(disp.clone()),
                    None,
                    None,
                );
            }
        }
    }

    if !can_create && wallet_connected && balance < 0.003 {
        ui.colored_label(egui::Color32::RED, "Insufficient balance (need ≥ 0.003 SOL)");
    }
}

#[cfg(feature = "solana")]
fn render_join_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
) {
    ui.label(egui::RichText::new("Enter Game ID:").size(14.0));
    ui.text_edit_singleline(&mut lobby.game_id_input);

    Layout::small_space(ui);

    let game_id_valid = lobby.game_id_input.trim().parse::<u64>().is_ok();
    let looking_up = matches!(lobby.status, LobbyStatus::Pending);
    let already_fetched = matches!(lobby.status, LobbyStatus::Fetched { .. });

    // Auto-lookup if pre-filled but not yet looked up
    if game_id_valid && !looking_up && !already_fetched && !lobby.game_id_input.is_empty() {
        if let Ok(game_id) = lobby.game_id_input.trim().parse::<u64>() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            spawn_lookup_game(lobby.cached_rpc_url.clone(), game_id, tx);
            lobby.lookup_rx = Some(rx);
            lobby.status = LobbyStatus::Pending;
            info!("[SOLANA_LOBBY] Automatic lookup for game {}", game_id);
        }
    }

    if ui.add_sized([ui.available_width(), 30.0], egui::Button::new(" Manual Look Up"))
        .clicked() && game_id_valid && !looking_up {
        if let Ok(game_id) = lobby.game_id_input.trim().parse::<u64>() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            spawn_lookup_game(lobby.cached_rpc_url.clone(), game_id, tx);
            lobby.lookup_rx = Some(rx);
            lobby.status = LobbyStatus::Pending;
            info!("[SOLANA_LOBBY] Manual lookup for game {}", game_id);
        }
    }

    // Show fetched wager info + confirm join button
    if let LobbyStatus::Fetched { wager_sol, game_id } = lobby.status {
        ui.separator();
        Layout::small_space(ui);
        ui.label(
            egui::RichText::new(format!("Game #{} requires {:.4} SOL wager", game_id, wager_sol))
                .color(egui::Color32::GOLD),
        );
        ui.label(
            egui::RichText::new(format!("Your balance: {:.4} SOL", lobby.cached_balance))
                .size(12.0),
        );

        let sufficient = lobby.cached_balance >= wager_sol + 0.002;
        let can_join = lobby.cached_keypair_bytes.is_some() && sufficient;

        if !sufficient {
            ui.colored_label(egui::Color32::RED, "Insufficient balance to join");
        }

        if ui.add_enabled(can_join, egui::Button::new(" Confirm Join")).clicked() {
            if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_join_game(
                    lobby.cached_rpc_url.clone(),
                    wallet_pubkey,
                    game_id,
                    tx,
                );
                // Copy fetched wager into create flow so poll_lobby_tasks can persist it
                lobby.wager_sol = wager_sol as f32;
                lobby.tx_rx = Some(rx);
                lobby.status = LobbyStatus::Pending;
                info!("[SOLANA_LOBBY] Joining game {} (wager {:.4} SOL)", game_id, wager_sol);
            }
        }
    }
}

#[cfg(feature = "solana")]
fn wallet_pubkey_from_cached(bytes: &Option<Vec<u8>>) -> Option<solana_sdk::pubkey::Pubkey> {
    let arr: [u8; 32] = bytes.as_deref()?.try_into().ok()?;
    Some(solana_sdk::pubkey::Pubkey::from(arr))
}

/// System to render a popup asking if the user wants to create a regular P2P lobby or a Solana wager lobby.
pub(super) fn render_lobby_selection_popup(
    mut contexts: bevy_egui::EguiContexts,
    mut menu_state: ResMut<NextState<crate::core::MenuState>>,
    _auth_state: ResMut<crate::ui::account::auth::AuthState>,
    #[cfg(feature = "solana")]
    solana_state: Option<Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>>,
) {
    let Some(ctx) = contexts.ctx_mut().ok() else { return };
    
    egui::Window::new("Create Multiplayer Lobby")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(320.0)
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(15, 15, 20))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(230, 57, 70)))
                .corner_radius(12.0)
                .inner_margin(24.0),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Choose Lobby Type")
                    .size(20.0)
                    .color(egui::Color32::from_rgb(230, 57, 70))
                    .strong());
                
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Select how you want to play against others.")
                    .color(egui::Color32::GRAY)
                    .size(13.0));
                
                ui.add_space(32.0);

                // Local Button
                let reg_btn = ui.add_sized(
                    [240.0, 48.0],
                    egui::Button::new(
                        egui::RichText::new("Local").size(16.0).strong()
                    )
                    .fill(egui::Color32::from_rgb(40, 40, 50))
                ).on_hover_text("Standard match without on-chain wagering");

                if reg_btn.clicked() {
                    menu_state.set(crate::core::MenuState::BraidLobby);
                }
                
                ui.add_space(16.0);
                
                // Solana Wager P2P Button (only available with solana feature)
                #[cfg(feature = "solana")]
                {
                    let solana_available = solana_state.as_ref().map(|s| s.wallet_pubkey.is_some()).unwrap_or(false);
                    let sol_btn = ui.add_sized(
                        [240.0, 48.0],
                        egui::Button::new(egui::RichText::new("Solana Wager P2P").size(16.0).strong())
                            .fill(if solana_available { egui::Color32::from_rgb(230, 57, 70) } else { egui::Color32::from_rgb(80, 80, 80) })
                    ).on_hover_text(if solana_available { "Play for SOL on-chain via Solana Devnet" } else { "Solana not available - feature disabled" });

                    if sol_btn.clicked() {
                        if let Some(state) = solana_state.as_ref() {
                            if state.wallet_pubkey.is_none() {
                                info!("[MENU] Solana wager blocked — wallet not connected. Opening wallet popup.");
                                tauri_signer::open_wallet_browser();
                            } else if state.profile_status != crate::multiplayer::solana::integration::state::ProfileStatus::HasProfileWithUsername {
                                info!("[MENU] Profile missing or incomplete. Redirecting to Profile Creation.");
                                menu_state.set(crate::core::MenuState::ProfileCreation);
                            } else {
                                menu_state.set(crate::core::MenuState::SolanaLobby);
                            }
                        }
                    }
                }
                
                #[cfg(not(feature = "solana"))]
                {
                    ui.add_enabled_ui(false, |ui| {
                        ui.add_sized(
                            [240.0, 48.0],
                            egui::Button::new(egui::RichText::new("Solana Wager P2P (Disabled)").size(16.0).strong())
                                .fill(egui::Color32::from_rgb(80, 80, 80))
                        ).on_hover_text("Solana feature not enabled in this build");
                    });
                }
                
                ui.add_space(32.0);
                
                if ui.button(egui::RichText::new("Cancel").color(egui::Color32::GRAY)).clicked() {
                    menu_state.set(crate::core::MenuState::Main);
                }
                
                ui.add_space(8.0);
            });
        });
}

/// Part 2F — P2P Braid Lobby screen shown when MenuState::BraidLobby is active.
pub(super) fn render_braid_lobby_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ctx.learn_viewport.rect_px = None;
    ui.vertical_centered(|ui| {
        ui.heading(egui::RichText::new("P2P LOBBY").size(24.0).color(egui::Color32::from_rgb(100, 200, 255)).strong());
        ui.add_space(8.0);

        if ui.button("⬅ Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::Main);
        }

        ui.add_space(16.0);

        // Node ID is internal only - not shown in UI
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Open Games").size(15.0).color(egui::Color32::WHITE).strong());
        ui.add_space(6.0);
        
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new(" Host P2P Game").color(egui::Color32::from_rgb(100, 255, 100)).strong()).clicked() {
                ctx.menu_state.set(crate::core::MenuState::HostConfig);
            }
            if ui.button(egui::RichText::new(" Refresh").size(14.0)).clicked() {
                if let Some(ref mut vps) = ctx.p2p_vps_state {
                    vps.last_poll = None; // Trigger immediate poll
                }
            }
        });
        ui.add_space(8.0);

        let games = ctx.p2p_vps_state.as_ref()
            .map(|v| v.cached_games.clone())
            .unwrap_or_else(Vec::new);

        if games.is_empty() {
            ui.label(egui::RichText::new("No open lobbies. Click 'Host P2P Game' to create one.").size(12.0).color(egui::Color32::GRAY).italics());
        } else {
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for game in &games {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(game.username.as_deref().unwrap_or_else(|| game.display_name.as_str())).size(13.0).color(egui::Color32::WHITE).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add_sized([60.0, 24.0], egui::Button::new(
                                    egui::RichText::new("Join").size(11.0).color(egui::Color32::WHITE).strong()
                                ).fill(egui::Color32::from_rgb(40, 140, 80)).corner_radius(4.0)).clicked() {
                                    info!("[MENU] Joining game: {}", game.game_id);
                                    let game_id = game.game_id.clone();
                                    let local_node_id = ctx.network_state.as_ref()
                                        .and_then(|ns| ns.node_id.as_ref().map(|id| bs58::encode(id.as_bytes()).into_string()))
                                        .unwrap_or_else(|| "unknown".to_string());
                                    
                                    let tx = if let Some(vps) = ctx.p2p_vps_state.as_ref() {
                                        Some(vps.response_tx.clone())
                                    } else { None };
                                    
                                    if let Some(tx) = tx {
                                        bevy::tasks::IoTaskPool::get().spawn(async move {
                                            match crate::multiplayer::network::vps::p2p_join_game(game_id.clone(), &local_node_id) {
                                                Ok(Some(host_id)) => {
                                                    let _ = tx.send(crate::multiplayer::network::p2p_vps::VpsResponse::JoinResult {
                                                        game_id,
                                                        host_node_id: Some(host_id),
                                                        stake_amount: 0.0,
                                                    });
                                                }
                                                Ok(None) => warn!("[MENU] Join rejected by VPS"),
                                                Err(e) => error!("[MENU] Join error: {}", e),
                                            }
                                        }).detach();
                                    }
                                }
                            });
                        });
                    });
                    ui.add_space(4.0);
                }
            });
        }
    });
}

/// Part 4B — Tournament browser screen shown when MenuState::Tournaments is active.
pub(super) fn render_tournament_browser_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ctx.learn_viewport.rect_px = None;
    ui.vertical_centered(|ui| {
        ui.heading(egui::RichText::new("TOURNAMENTS").size(24.0).color(egui::Color32::from_rgb(255, 200, 50)).strong());
        ui.add_space(8.0);

        if ui.button("⬅ Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::Main);
        }

        ui.add_space(16.0);

        #[cfg(feature = "solana")]
        {
            let wallet_pubkey = ctx.solana_state.as_ref()
                .and_then(|s| s.wallet_pubkey.map(|p| p.to_string()));

            let tournaments = ctx.tournament_client.as_ref()
                .map(|tc| tc.available_tournaments.clone())
                .unwrap_or_else(Vec::new);

            if tournaments.is_empty() {
                ui.label(egui::RichText::new("No tournaments available. Check back later.").size(13.0).color(egui::Color32::GRAY).italics());
            } else {
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    for t in &tournaments {
                        let is_password_prompt = ctx.tournament_client.as_ref()
                            .map(|tc| tc.active_tournament_id == Some(t.tournament_id) && t.is_private)
                            .unwrap_or(false);

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(&t.name).size(14.0).color(egui::Color32::WHITE).strong());
                                    let entry = if t.entry_fee_lamports == 0 {
                                        "Free entry".to_string()
                                    } else {
                                        format!("Entry: {:.3} SOL", t.entry_fee_lamports as f64 / 1e9)
                                    };
                                    ui.label(egui::RichText::new(entry).size(11.0).color(egui::Color32::from_rgb(150, 200, 150)));
                                    ui.label(egui::RichText::new(format!("{} players  |  {}", t.registered, t.status)).size(10.0).color(egui::Color32::GRAY));
                                    // Type label
                                    let type_label = if t.is_tournament {
                                        " Tournament"
                                    } else if t.entry_fee_lamports > 0 {
                                        " Wagered Game"
                                    } else {
                                        " Free Game"
                                    };
                                    let type_color = if t.is_tournament {
                                        egui::Color32::from_rgb(255, 215, 0) // Gold
                                    } else if t.entry_fee_lamports > 0 {
                                        egui::Color32::from_rgb(230, 57, 70) // Red/pink
                                    } else {
                                        egui::Color32::from_rgb(100, 200, 100) // Green
                                    };
                                    ui.label(egui::RichText::new(type_label).size(10.0).color(type_color).strong());
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let can_join = wallet_pubkey.is_some();
                                    let join_btn = ui.add_enabled(can_join && !is_password_prompt, egui::Button::new(
                                        egui::RichText::new("Join").size(12.0).color(egui::Color32::WHITE).strong()
                                    ).fill(if can_join { egui::Color32::from_rgb(50, 140, 50) } else { egui::Color32::from_rgb(60, 60, 60) }).corner_radius(4.0).min_size(egui::vec2(60.0, 28.0)));
                                    if t.is_private {
                                        ui.label(egui::RichText::new("").size(14.0).color(egui::Color32::WHITE));
                                    }
                                    if join_btn.clicked() {
                                        let tid = t.tournament_id;
                                        if let Some(ref mut tc) = ctx.tournament_client {
                                            tc.active_tournament_id = Some(tid);
                                            if t.is_private {
                                                tc.password_input = String::new();
                                                tc.password_error = None;
                                            } else {
                                                let pk = match wallet_pubkey.clone() {
                                                    Some(pk) => pk,
                                                    None => {
                                                        warn!("[TOURNAMENT] Cannot join: wallet not connected");
                                                        return;
                                                    }
                                                };
                                                bevy::tasks::IoTaskPool::get().spawn(async move {
                                                    match crate::multiplayer::network::vps::join_tournament(tid, &pk, None) {
                                                        Ok(slot) => info!("[TOURNAMENT] Joined tournament {} slot {}", tid, slot),
                                                        Err(e) => warn!("[TOURNAMENT] Join failed: {}", e),
                                                    }
                                                }).detach();
                                                tc.join_status = crate::multiplayer::solana::tournament::TournamentJoinStatus::Pending;
                                            }
                                        }
                                    }
                                });
                            });

                            // Inline password prompt for private tournaments
                            if is_password_prompt {
                                if let Some(ref mut tc) = ctx.tournament_client {
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Password:").size(12.0).color(egui::Color32::WHITE));
                                        ui.add(egui::TextEdit::singleline(&mut tc.password_input).password(true).desired_width(120.0));
                                        if ui.button(egui::RichText::new("Submit").size(12.0).color(egui::Color32::WHITE).strong()).clicked() {
                                            let tid = t.tournament_id;
                                            let pk = wallet_pubkey.clone().unwrap_or_default();
                                            let password = tc.password_input.clone();
                                            bevy::tasks::IoTaskPool::get().spawn(async move {
                                                match crate::multiplayer::network::vps::join_tournament(tid, &pk, Some(&password)) {
                                                    Ok(slot) => info!("[TOURNAMENT] Joined private tournament {} slot {}", tid, slot),
                                                    Err(e) => warn!("[TOURNAMENT] Private join failed: {}", e),
                                                }
                                            }).detach();
                                            tc.join_status = crate::multiplayer::solana::tournament::TournamentJoinStatus::Pending;
                                            tc.password_error = None;
                                        }
                                        if ui.button(egui::RichText::new("Cancel").size(12.0).color(egui::Color32::WHITE)).clicked() {
                                            tc.active_tournament_id = None;
                                            tc.password_input = String::new();
                                            tc.password_error = None;
                                        }
                                    });
                                    if let Some(ref err) = tc.password_error {
                                        ui.label(egui::RichText::new(err).size(11.0).color(egui::Color32::from_rgb(255, 100, 100)));
                                    }
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }
                });
            }
        }
        #[cfg(not(feature = "solana"))]
        ui.label(egui::RichText::new("Tournament browser requires the solana feature.").size(13.0).color(egui::Color32::GRAY).italics());
    });
}

pub(super) fn render_host_p2p_config_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        ui.heading(egui::RichText::new("HOST P2P GAME").size(24.0).color(egui::Color32::from_rgb(100, 200, 255)).strong());
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.label(egui::RichText::new("Time Control").size(15.0).color(egui::Color32::WHITE).strong());
            ui.add_space(8.0);
            
            ui.horizontal(|ui| {
                ui.label("Minutes per side:");
                ui.add(egui::Slider::new(&mut ctx.p2p_host.base_time_minutes, 1..=60));
            });
            
            ui.horizontal(|ui| {
                ui.label("Increment (seconds):");
                ui.add(egui::Slider::new(&mut ctx.p2p_host.increment_seconds, 0..=60));
            });
        });

        ui.add_space(12.0);

        ui.group(|ui| {
            ui.label(egui::RichText::new("Wager (Coming Soon)").size(15.0).color(egui::Color32::GRAY).strong());
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Currently only free P2P games are supported.").size(12.0).color(egui::Color32::GRAY).italics());
        });

        ui.add_space(24.0);

        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("Cancel").size(16.0)).clicked() {
                ctx.menu_state.set(crate::core::MenuState::BraidLobby);
            }

            ui.add_space(12.0);

            let node_id_ready = ctx.network_state.as_ref().map(|ns| ns.node_id.is_some()).unwrap_or(false);
            
            let start_btn = ui.add_enabled(node_id_ready, egui::Button::new(
                egui::RichText::new(" Start Hosting").size(18.0).color(egui::Color32::WHITE).strong()
            ).fill(egui::Color32::from_rgb(40, 140, 80)));
            
            if start_btn.clicked() {
                // Generate Gam lse(|| "nknown".to_sring)
                let game_id = format!("p2p_{}", rand::random::<u32>());
                ctx.p2p_host.game_id = Some(game_id.clone());

                // Firing events for internal systems
                if let Some(host_events) = &mut ctx.host_game_events {
                    host_events.write(crate::multiplayer::network::p2p::HostGameEvent);
                }

                // Announce to VPS
                let display_name = ctx.player_identity.display_name().to_string();
                let host_node_id = ctx.network_state.as_ref()
                    .and_then(|ns| ns.node_id.map(|id| bs58::encode(id.as_bytes()).into_string()))
                    .unwrap_or_default();
                
                let _ = crate::multiplayer::vps_client::p2p_announce_game(
                    game_id.clone(),
                    &host_node_id,
                    &display_name,
                    ctx.p2p_host.stake_amount,
                    "P2P",
                    (ctx.p2p_host.base_time_minutes * 60) as u32,
                    ctx.p2p_host.increment_seconds as u16,
                    Some(display_name.clone()),
                    None,
                    None,
                );
                
                info!("[LOBBY] Hosting P2P game: {} ({} + {})", game_id, ctx.p2p_host.base_time_minutes, ctx.p2p_host.increment_seconds);
                
                // Transition to Waiting Screen
                ctx.menu_state.set(crate::core::MenuState::P2PWaiting);
                
                // Also update internal connection state
                if let Some(ref mut p2p_state) = ctx.p2p_state {
                    p2p_state.status = P2PConnectionStatus::Hosting;
                }
            }
            
            if !node_id_ready {
                ui.label(egui::RichText::new("Wait for P2P initialization…").size(11.0).color(egui::Color32::RED));
            }
        });
    });
}

pub(super) fn render_p2p_waiting_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.heading(egui::RichText::new("WAITING FOR OPPONENT").size(24.0).color(egui::Color32::GOLD).strong());
        ui.add_space(20.0);

        ui.label(egui::RichText::new("Your game is now visible in the lobby.").size(14.0).color(egui::Color32::WHITE));
        ui.add_space(8.0);
        
        if let Some(game_id) = &ctx.p2p_host.game_id {
            ui.label(egui::RichText::new(format!("Game ID: {}", game_id)).size(12.0).color(egui::Color32::GRAY).monospace());
        }
        ui.add_space(30.0);
        
        // Simple animated dots
        ui.label(egui::RichText::new("• • •").size(32.0).color(egui::Color32::GOLD));
        
        ui.add_space(40.0);

        if ui.button(egui::RichText::new(" Cancel Hosting").size(16.0).color(egui::Color32::from_rgb(255, 100, 100))).clicked() {
            // Cancel on VPS
            if let Some(game_id) = &ctx.p2p_host.game_id {
                let node_id = ctx.network_state.as_ref()
                    .and_then(|ns| ns.node_id.map(|id| bs58::encode(id.as_bytes()).into_string()))
                    .unwrap_or_default();
                    
                let _ = crate::multiplayer::vps_client::p2p_leave_game(game_id.clone(), &node_id);
                info!("[LOBBY] Cancelled hosting for {}", game_id);
            }
            
            // Reset state
            ctx.p2p_host.game_id = None;
            if let Some(ref mut p2p_state) = ctx.p2p_state {
                p2p_state.status = P2PConnectionStatus::Disconnected;
            }
            
            ctx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
    });
}

