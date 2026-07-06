//! Main menu screen and popup rendering helpers.
//!
//! This module contains the UI that was split out of `src/states/main_menu.rs`
//! so the main menu state stays focused on plugin setup and high-level flow.
//! The parent module calls into these helpers to render the Solana lobby,
//! braid lobby, tournament browser, host configuration, waiting screens, and
//! popups while keeping the shared state in `MainMenuUIContext`.

use super::*;
#[cfg(feature = "solana")]
use crate::core::GameMode as CoreGameMode;
#[cfg(feature = "solana")]
use crate::game::ai::GameMode as AIGameMode;
use crate::multiplayer::network::p2p::P2PConnectionStatus;
#[cfg(feature = "solana")]
use crate::ui::styles::Layout;
use bevy_egui::egui;
use tracing::{error, info, warn};

#[cfg(feature = "solana")]
use crate::multiplayer::solana::lobby::{
    spawn_create_game, spawn_join_game, spawn_lookup_game, spawn_poll_opponent_joined, LobbyMode,
    LobbyStatus,
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
        ns.node_id
            .map(|id| bs58::encode(id.as_bytes()).into_string())
    });

    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        if ui.button("? Back").clicked() {
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

        // Rejoin banner: show if an active game was found for this wallet
        if let Some(rejoin_id) = lobby.rejoin_game_id {
            ui.separator();
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 220, 50),
                    format!("? Active game found (ID: {})", rejoin_id),
                );
                if ui.button("Rejoin").clicked() {
                    if let Some(ref mut sync) = ctx.solana_sync {
                        sync.game_id = Some(rejoin_id);
                    }
                    if let Some(ref mut comp) = ctx.competitive {
                        comp.game_id = Some(rejoin_id);
                        comp.active = true;
                    }
                    crate::multiplayer::network::game_id_store::set(rejoin_id);
                    ctx.ai_config.mode = crate::game::ai::resource::GameMode::Multiplayer;
                    *ctx.core_mode = crate::core::GameMode::OnlineMultiplayer;
                    ctx.menu_state.set(crate::core::MenuState::Main);
                }
                if ui.small_button("Dismiss").clicked() {
                    lobby.rejoin_game_id = None;
                }
            });
            ui.separator();
        }

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
                if ui
                    .selectable_label(lobby.mode == LobbyMode::Create, "Create Game")
                    .clicked()
                {
                    lobby.mode = LobbyMode::Create;
                    lobby.status = LobbyStatus::Idle;
                }
                if ui
                    .selectable_label(lobby.mode == LobbyMode::Join, "Join by ID")
                    .clicked()
                {
                    lobby.mode = LobbyMode::Join;
                    lobby.status = LobbyStatus::Idle;
                }
                if ui
                    .selectable_label(lobby.mode == LobbyMode::Browse, "Browse Games")
                    .clicked()
                {
                    lobby.mode = LobbyMode::Browse;
                    lobby.status = LobbyStatus::Idle;
                    lobby.browse_last_fetch = None; // trigger immediate refresh
                }
            });

            ui.separator();
            Layout::item_space(ui);

            let node_id_b58 = ctx.network_state.as_ref().and_then(|ns| {
                ns.node_id
                    .map(|id| bs58::encode(id.as_bytes()).into_string())
            });
            let gbp_per_sol = ctx
                .sol_gbp_rate
                .as_ref()
                .and_then(|r| r.current.as_ref())
                .map(|s| s.gbp_per_sol);
            match lobby.mode {
                LobbyMode::Create => render_create_tab(
                    ui,
                    lobby,
                    &mut ctx.compliance,
                    node_id_b58.as_deref(),
                    gbp_per_sol,
                ),
                LobbyMode::Join => render_join_tab(ui, lobby, node_id_b58.as_deref(), gbp_per_sol),
                LobbyMode::Browse => render_solana_browse_tab(ui, lobby),
            }
        }

        Layout::item_space(ui);

        // Auto-transition: Success + Create mode ? WaitingForOpponent + start poll.
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
                    egui::RichText::new("? Submitting transaction...")
                        .color(egui::Color32::from_rgb(200, 200, 50)),
                );
            }

            LobbyStatus::WaitingForOpponent { game_id } => {
                ui.spinner();
                ui.label(
                    egui::RichText::new("? Waiting for opponent to join...")
                        .color(egui::Color32::from_rgb(255, 200, 80)),
                );
                Layout::small_space(ui);

                // Game ID with copy button
                if game_id > 0 {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("Game ID: {}", game_id)).strong());
                        if ui
                            .small_button("Copy")
                            .on_hover_text("Copy game ID to clipboard")
                            .clicked()
                        {
                            ui.output_mut(|o| {
                                o.commands
                                    .push(egui::OutputCommand::CopyText(game_id.to_string()))
                            });
                        }
                    });
                }

                // Pot preview
                let wager = lobby.wager_sol;
                if wager > 0.0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "Your escrow: {:.4} SOL  |  Total pot: {:.4} SOL",
                            wager,
                            wager * 2.0
                        ))
                        .color(egui::Color32::GOLD)
                        .size(12.0),
                    );
                }

                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new(
                        "Share your Game ID with a friend or wait for someone from the lobby list.",
                    )
                    .size(12.0)
                    .color(egui::Color32::LIGHT_GRAY),
                );

                // Pre-game lobby chat
                let game_id_str = game_id.to_string();
                if !ctx.lobby_chat.active || ctx.lobby_chat.game_id != game_id_str {
                    let backend_url = std::env::var("BACKEND_URL")
                        .or_else(|_| std::env::var("SIGNING_SERVICE_URL"))
                        .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string());
                    let display = lobby
                        .cached_display_name
                        .clone()
                        .unwrap_or_else(|| "Anonymous".to_string());
                    ctx.lobby_chat
                        .activate(game_id_str.clone(), backend_url, display);
                }
                ui.add_space(6.0);
                ui.collapsing(
                    egui::RichText::new("Pre-game Chat")
                        .size(12.0)
                        .color(egui::Color32::LIGHT_GRAY),
                    |ui| {
                        let msgs = ctx.lobby_chat.messages.clone();
                        egui::ScrollArea::vertical()
                            .max_height(100.0)
                            .id_salt("lobby_chat_scroll")
                            .show(ui, |ui| {
                                if msgs.is_empty() {
                                    ui.label(
                                        egui::RichText::new("No messages yet.")
                                            .size(11.0)
                                            .color(egui::Color32::GRAY)
                                            .italics(),
                                    );
                                }
                                for msg in &msgs {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}: {}",
                                            msg.player, msg.text
                                        ))
                                        .size(11.0),
                                    );
                                }
                            });
                        ui.horizontal(|ui| {
                            let draft_resp = ui.add(
                                egui::TextEdit::singleline(&mut ctx.lobby_chat.draft)
                                    .desired_width(180.0)
                                    .hint_text("Say something..."),
                            );
                            let send = ui.small_button("Send").clicked()
                                || (draft_resp.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                            if send && !ctx.lobby_chat.draft.is_empty() {
                                let text = ctx.lobby_chat.draft.clone();
                                let display = lobby
                                    .cached_display_name
                                    .clone()
                                    .unwrap_or_else(|| "Me".to_string());
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis()
                                    as u64;
                                ctx.lobby_chat.messages.push(
                                    crate::multiplayer::social::LobbyMsg {
                                        player: display.clone(),
                                        text: text.clone(),
                                        timestamp_ms: now_ms,
                                    },
                                );
                                ctx.lobby_chat.draft.clear();
                                let gid = game_id_str.clone();
                                let backend_url = std::env::var("BACKEND_URL")
                                    .or_else(|_| std::env::var("SIGNING_SERVICE_URL"))
                                    .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string());
                                std::thread::spawn(move || {
                                    let _ = reqwest::blocking::Client::new()
                                        .post(format!("{}/chat/{}", backend_url, gid))
                                        .json(
                                            &serde_json::json!({ "player": display, "text": text }),
                                        )
                                        .send();
                                });
                            }
                        });
                    },
                );

                Layout::small_space(ui);
                if ui.small_button(" Cancel").clicked() {
                    ctx.lobby_chat.deactivate();
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
                    egui::RichText::new("Click 'Host Game' to start the match.")
                        .size(12.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );
                Layout::small_space(ui);
                if ui.button(" Host Game").clicked() {
                    start_solana_braid_game(
                        &mut ctx.online_session,
                        &ctx.network_config,
                        ctx.network_state.as_ref().map(|r| &**r),
                        ctx.p2p_state.as_deref_mut(),
                        game_id,
                        true,
                        wager_lamports,
                    );
                    ctx.ai_config.mode = AIGameMode::Multiplayer;
                    *ctx.core_mode = CoreGameMode::OnlineMultiplayer;
                    if let Some(ref mut sync) = ctx.solana_sync {
                        sync.game_id = Some(game_id);
                        sync.wager_amount = wager_lamports;
                    }
                    if let Some(ref mut comp) = ctx.competitive {
                        comp.game_id = Some(game_id);
                        comp.wager_lamports = wager_lamports;
                        comp.active = true;
                    }
                    ctx.game_started_events
                        .write(crate::game::events::GameStartedEvent { game_id });
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
                    let is_host = lobby.mode == LobbyMode::Create;
                    start_solana_braid_game(
                        &mut ctx.online_session,
                        &ctx.network_config,
                        ctx.network_state.as_ref().map(|r| &**r),
                        ctx.p2p_state.as_deref_mut(),
                        game_id,
                        is_host,
                        wager_lamports,
                    );
                    ctx.ai_config.mode = AIGameMode::Multiplayer;
                    *ctx.core_mode = CoreGameMode::OnlineMultiplayer;
                    if let Some(ref mut sync) = ctx.solana_sync {
                        sync.game_id = Some(game_id);
                        sync.wager_amount = wager_lamports;
                    }
                    if let Some(ref mut comp) = ctx.competitive {
                        comp.game_id = Some(game_id);
                        comp.wager_lamports = wager_lamports;
                        comp.active = true;
                    }
                    ctx.game_started_events
                        .write(crate::game::events::GameStartedEvent { game_id });
                    ctx.menu_state.set(crate::core::MenuState::Main);
                }
            }

            LobbyStatus::Error(msg) => {
                ui.colored_label(egui::Color32::RED, format!(" {}", msg));
                if ui.small_button("? Try Again").clicked() {
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
    spectate_writer: &mut Option<
        crate::multiplayer::traits::MessageWriter<
            crate::multiplayer::spectator::SpectateViaLinkEvent,
        >,
    >,
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
                                        if s == 0 { format!("? {}+{}", m, game.increment_seconds) }
                                        else { format!("? {}s+{}", game.base_time_seconds, game.increment_seconds) }
                                    } else { "? 8".to_string() };
                                    ui.label(egui::RichText::new(tc_label).size(10.0).color(egui::Color32::GRAY));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let game_id = game.game_id.clone();
                                    if ui.add_sized([60.0, 28.0], egui::Button::new(
                                        egui::RichText::new("Watch").size(11.0).color(egui::Color32::WHITE).strong()
                                    ).fill(egui::Color32::from_rgb(60, 100, 160)).corner_radius(4.0)).clicked() {
                                        if let Some(ref mut w) = spectate_writer {
                                            w.write(crate::multiplayer::spectator::SpectateViaLinkEvent { game_id: game_id.clone() });
                                            competitive.show_spectator_popup = false;
                                        }
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

/// Render join lobby popup — kept for legacy callers; no longer wired to a button.

#[cfg(feature = "solana")]
fn render_create_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    compliance: &mut crate::ui::compliance_modal::ComplianceState,
    node_id: Option<&str>,
    gbp_per_sol: Option<f64>,
) {
    use crate::multiplayer::solana::lobby::EloMatchPref;

    ui.heading(
        egui::RichText::new("Create Game")
            .size(22.0)
            .color(egui::Color32::from_rgb(100, 200, 255))
            .strong(),
    );
    ui.add_space(12.0);

    let balance = lobby.cached_balance;
    let wallet_connected = lobby.cached_keypair_bytes.is_some();
    let max_wager = if wallet_connected {
        ((balance - 0.002) as f32).max(0.0)
    } else {
        0.0
    };

    // Session key status chip
    if wallet_connected {
        let now = chrono::Utc::now().timestamp();
        let (chip_text, chip_color) = match lobby.session_expires_at {
            Some(exp) if exp > now + 86400 => (
                format!("Session: Authorized ({}h left)", (exp - now) / 3600),
                egui::Color32::from_rgb(34, 197, 94),
            ),
            Some(exp) if exp > now => {
                let hours = (exp - now) / 3600;
                (
                    format!("Session: Expiring soon (~{}h)", hours),
                    egui::Color32::from_rgb(251, 191, 36),
                )
            }
            Some(_) => (
                "Session: Expired".to_string(),
                egui::Color32::from_rgb(239, 68, 68),
            ),
            None => ("Session: Not authorized".to_string(), egui::Color32::GRAY),
        };
        ui.horizontal(|ui| {
            ui.colored_label(chip_color, &chip_text);
            if matches!(lobby.session_expires_at, None | Some(_) if lobby.session_expires_at.map_or(true, |e| e <= now + 86400)) {
                if ui.small_button("Authorize").on_hover_text("Re-authorize session key for this wallet").clicked() {
                    info!("[LOBBY] User triggered session key re-authorization");
                }
            }
        });
        Layout::small_space(ui);
    }

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

    // GBP label + rough USD (1 GBP ≈ 1.27 USD)
    let fiat_str = gbp_per_sol
        .map(|rate| {
            let gbp = lobby.wager_sol as f64 * rate;
            let usd = gbp * 1.27;
            format!(" (£{:.2} / ~${:.2})", gbp, usd)
        })
        .unwrap_or_default();
    let label_text = if lobby.wager_sol == 0.0 && lobby.match_type == 1 {
        "Free Rated game — ELO tracked, no SOL at stake".to_string()
    } else if lobby.wager_sol == 0.0 {
        "Free casual game — no SOL at stake".to_string()
    } else {
        format!(
            "Escrow: {:.4} SOL{}  |  Pot: {:.4} SOL",
            lobby.wager_sol,
            fiat_str,
            lobby.wager_sol * 2.0
        )
    };
    ui.label(
        egui::RichText::new(label_text)
            .color(egui::Color32::LIGHT_GRAY)
            .size(12.0),
    );

    Layout::small_space(ui);

    // Time control selector
    ui.label(egui::RichText::new("Time Control").size(13.0));
    ui.horizontal_wrapped(|ui| {
        for (label, base, inc) in [
            ("Bullet 1+0", 60u32, 0u32),
            ("Blitz 3+2", 180, 2),
            ("Blitz 5+0", 300, 0),
            ("Rapid 10+0", 600, 0),
            ("Rapid 15+10", 900, 10),
            ("30 min", 1800, 0),
        ] {
            let selected = lobby.time_control_base == base && lobby.time_control_inc == inc;
            if ui.selectable_label(selected, label).clicked() {
                lobby.time_control_base = base;
                lobby.time_control_inc = inc;
            }
        }
    });

    Layout::small_space(ui);

    // ELO matching preference (only relevant for rated/wagered games)
    if lobby.match_type >= 1 || lobby.wager_sol > 0.0 {
        ui.label(egui::RichText::new("ELO Matching").size(13.0));
        ui.horizontal(|ui| {
            for pref in [
                EloMatchPref::Strict,
                EloMatchPref::Expanded,
                EloMatchPref::Any,
            ] {
                if ui
                    .selectable_label(lobby.elo_pref == pref, pref.label())
                    .clicked()
                {
                    lobby.elo_pref = pref;
                }
            }
        });
        Layout::small_space(ui);
    }

    Layout::small_space(ui);

    let is_free_casual = lobby.wager_sol == 0.0 && lobby.match_type == 0;
    let is_free_rated = lobby.wager_sol == 0.0 && lobby.match_type == 1;
    let can_create = !matches!(lobby.status, LobbyStatus::Pending)
        && (is_free_casual
            || (is_free_rated && wallet_connected)
            || (wallet_connected
                && lobby.wager_sol > 0.0
                && (lobby.wager_sol as f64) <= balance - 0.002))
        && !matches!(lobby.status, LobbyStatus::Pending);

    let is_devnet = lobby.cached_rpc_url.contains("devnet");

    let create_btn_text = if is_free_casual {
        " Host Free Game"
    } else if is_free_rated {
        " Host Free Rated Game"
    } else {
        " Create Wagered Game"
    };

    if ui
        .add_sized(
            [ui.available_width(), 40.0],
            egui::Button::new(egui::RichText::new(create_btn_text).size(16.0).strong()).fill(
                if can_create {
                    egui::Color32::from_rgb(40, 100, 40)
                } else {
                    egui::Color32::from_rgb(40, 40, 40)
                },
            ),
        )
        .clicked()
        && can_create
    {
        if is_free_casual {
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
                    info!(
                        "[LOBBY] Free game announced (id={}). Waiting for opponent via VPS.",
                        game_id
                    );
                    lobby.status = LobbyStatus::WaitingForOpponent { game_id: 0 };
                }
                Err(e) => {
                    warn!(
                        "[LOBBY] VPS announce failed ({}). Starting locally anyway.",
                        e
                    );
                    lobby.status = LobbyStatus::WaitingForOpponent { game_id: 0 };
                }
            }
        } else {
            // Wagered game: enforce CARF compliance on mainnet, then spawn Solana TX.
            if !is_devnet
                && compliance.status != crate::ui::compliance_modal::SubmissionStatus::Success
            {
                compliance.show = true;
                if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes)
                {
                    compliance.pubkey = Some(wallet_pubkey.to_string());
                }
            } else if let Some(wallet_pubkey) =
                wallet_pubkey_from_cached(&lobby.cached_keypair_bytes)
            {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_create_game(
                    lobby.cached_rpc_url.clone(),
                    wallet_pubkey,
                    lobby.wager_lamports(),
                    lobby.match_type,
                    lobby.time_control_base,
                    lobby.time_control_inc,
                    tx,
                );
                lobby.tx_rx = Some(rx);
                lobby.status = LobbyStatus::Pending;
                info!(
                    "[SOLANA_LOBBY] Creating wagered game ({} SOL)",
                    lobby.wager_sol
                );
            }
        }
    }

    if !can_create && wallet_connected && balance < 0.003 {
        ui.colored_label(
            egui::Color32::RED,
            "Insufficient balance (need = 0.003 SOL)",
        );
    }
}

#[cfg(feature = "solana")]
fn render_join_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
    node_id: Option<&str>,
    gbp_per_sol: Option<f64>,
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

    if ui
        .add_sized(
            [ui.available_width(), 30.0],
            egui::Button::new(" Manual Look Up"),
        )
        .clicked()
        && game_id_valid
        && !looking_up
    {
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
        let gbp_str = gbp_per_sol
            .map(|r| format!(" (£{:.2})", wager_sol * r))
            .unwrap_or_default();
        ui.label(
            egui::RichText::new(format!(
                "Game #{} requires {:.4} SOL{} wager",
                game_id, wager_sol, gbp_str
            ))
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

        if ui
            .add_enabled(can_join, egui::Button::new(" Confirm Join"))
            .clicked()
        {
            if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                if let Some(node_id) = node_id {
                    match crate::multiplayer::vps_client::p2p_join_game(
                        game_id.to_string(),
                        node_id,
                    ) {
                        Ok(Some(host_id)) => info!(
                            "[SOLANA_LOBBY] Relay join accepted for game {} (host {})",
                            game_id, host_id
                        ),
                        Ok(None) => warn!(
                            "[SOLANA_LOBBY] Relay join accepted without host node for game {}",
                            game_id
                        ),
                        Err(e) => warn!("[SOLANA_LOBBY] Relay join failed for {}: {}", game_id, e),
                    }
                }
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_join_game(lobby.cached_rpc_url.clone(), wallet_pubkey, game_id, tx);
                // Copy fetched wager into create flow so poll_lobby_tasks can persist it
                lobby.wager_sol = wager_sol as f32;
                lobby.tx_rx = Some(rx);
                lobby.status = LobbyStatus::Pending;
                info!(
                    "[SOLANA_LOBBY] Joining game {} (wager {:.4} SOL)",
                    game_id, wager_sol
                );
            }
        }
    }
}

#[cfg(feature = "solana")]
fn start_solana_braid_game(
    online_session: &mut crate::multiplayer::network::online_game_session::OnlineGameSession,
    network_config: &crate::multiplayer::types::NetworkConfig,
    network_state: Option<&crate::multiplayer::types::OnlineNetworkState>,
    p2p_state: Option<&mut crate::multiplayer::network::p2p::P2PConnectionState>,
    game_id: u64,
    is_host: bool,
    wager_lamports: u64,
) {
    if let Some(network_state) = network_state {
        crate::multiplayer::network::online_game_session::start_session(
            online_session,
            network_config.vps_base_url.clone(),
            game_id.to_string(),
            wager_lamports as f64 / 1_000_000_000.0,
            network_state,
        );
    }

    if let Some(p2p) = p2p_state {
        p2p.is_host = is_host;
        p2p.player_color = Some(if is_host {
            crate::rendering::pieces::PieceColor::White
        } else {
            crate::rendering::pieces::PieceColor::Black
        });
        p2p.status = P2PConnectionStatus::InGame;
    }
}

#[cfg(feature = "solana")]
fn render_solana_browse_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
) {
    use crate::multiplayer::solana::lobby::{spawn_join_game, LobbyStatus};

    let is_loading = lobby.browse_rx.is_some();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Open Wagered Games")
                .size(14.0)
                .strong()
                .color(egui::Color32::GOLD),
        );
        if is_loading {
            ui.spinner();
        } else if ui.small_button("Refresh").clicked() {
            lobby.browse_last_fetch = None;
        }
    });
    ui.add_space(6.0);

    if is_loading && lobby.browse_games.is_empty() {
        ui.label(
            egui::RichText::new("Loading…")
                .size(12.0)
                .color(egui::Color32::GRAY)
                .italics(),
        );
        return;
    }

    let games = lobby.browse_games.clone();
    let wagered: Vec<_> = games.iter().filter(|g| g.stake_amount > 0.0).collect();
    if wagered.is_empty() {
        ui.label(
            egui::RichText::new("No open wagered games right now.")
                .size(12.0)
                .color(egui::Color32::GRAY),
        );
        return;
    }

    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
        for game in &wagered {
            let name = game.username.as_deref().unwrap_or(game.display_name.as_str());
            let mins = game.base_time_seconds / 60;
            let inc  = game.increment_seconds;
            let time_str = if inc > 0 { format!("{}+{}", mins, inc) } else { format!("{} min", mins) };

            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(40, 35, 20, 220))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 80, 20)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(name).size(13.0).color(egui::Color32::WHITE).strong());
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&time_str).size(11.0).color(egui::Color32::from_rgb(160, 190, 255)));
                                ui.label(egui::RichText::new("·").size(11.0).color(egui::Color32::GRAY));
                                ui.label(egui::RichText::new(format!("{:.3} SOL", game.stake_amount)).size(11.0).color(egui::Color32::GOLD));
                                if let Some(elo) = game.elo {
                                    ui.label(egui::RichText::new("·").size(11.0).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("{} ELO", elo)).size(11.0).color(egui::Color32::from_rgb(200, 160, 255)));
                                }
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let full = game.players_joined >= game.capacity;
                            let can_join = lobby.cached_keypair_bytes.is_some()
                                && lobby.cached_balance >= game.stake_amount + 0.002
                                && !full;
                            if ui.add_enabled(can_join, egui::Button::new(
                                egui::RichText::new(if full { "Full" } else { "Join" }).size(12.0).strong()
                            ).fill(if can_join { egui::Color32::from_rgb(140, 80, 0) } else { egui::Color32::from_rgb(60, 60, 60) }).corner_radius(4.0).min_size(egui::vec2(60.0, 28.0))).clicked() {
                                if let Some(pk) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                                    if let Ok(game_id) = game.game_id.rsplit('_').next().and_then(|s| s.parse::<u64>().ok()).ok_or("") {
                                        if let Some(node_id) = &lobby.cached_node_id {
                                            match crate::multiplayer::vps_client::p2p_join_game(game.game_id.clone(), node_id) {
                                                Ok(Some(host_id)) => info!("[SOLANA_BROWSE] Relay join accepted for game {} (host {})", game.game_id, host_id),
                                                Ok(None) => warn!("[SOLANA_BROWSE] Relay join accepted without host node for game {}", game.game_id),
                                                Err(e) => warn!("[SOLANA_BROWSE] Relay join failed for {}: {}", game.game_id, e),
                                            }
                                        }
                                        let (tx, rx) = tokio::sync::oneshot::channel();
                                        spawn_join_game(lobby.cached_rpc_url.clone(), pk, game_id, tx);
                                        lobby.wager_sol = game.stake_amount as f32;
                                        lobby.tx_rx = Some(rx);
                                        lobby.status = LobbyStatus::Pending;
                                        lobby.game_id_input = game_id.to_string();
                                        lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Join;
                                    }
                                }
                            }
                        });
                    });
                });
            ui.add_space(4.0);
        }
    });
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
    #[cfg(feature = "solana")] solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
) {
    let Some(ctx) = contexts.ctx_mut().ok() else {
        return;
    };

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
                                info!("[MENU] Profile missing or incomplete. Opening Tauri profile step.");
                                std::thread::spawn(|| {
                                    let _ = reqwest::blocking::Client::new()
                                        .post("http://127.0.0.1:7454/api/open-profile-step")
                                        .send();
                                });
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

/// Part 2F — P2P lobby browser (MenuState::BraidLobby).
pub(super) fn render_braid_lobby_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ctx.learn_viewport.rect_px = None;

    ui.horizontal(|ui| {
        if ui.button(egui::RichText::new("Back").size(13.0)).clicked() {
            ctx.menu_state.set(crate::core::MenuState::Main);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(egui::RichText::new("⟳ Refresh").size(13.0))
                .clicked()
            {
                if let Some(ref mut vps) = ctx.p2p_vps_state {
                    vps.last_poll = None;
                }
            }
            if ui
                .add_sized(
                    [110.0, 26.0],
                    egui::Button::new(
                        egui::RichText::new("+ Create Lobby")
                            .size(13.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .fill(egui::Color32::from_rgb(40, 120, 60))
                    .corner_radius(4.0),
                )
                .clicked()
            {
                ctx.menu_state.set(crate::core::MenuState::HostConfig);
            }
        });
    });

    ui.add_space(8.0);

    // Join by code
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Join by code:")
                .size(13.0)
                .color(egui::Color32::WHITE),
        );
        ui.add(
            egui::TextEdit::singleline(&mut ctx.competitive_menu.join_game_id)
                .hint_text("p2p_xxxxxxxx")
                .desired_width(160.0),
        );
        let can_join = !ctx.competitive_menu.join_game_id.trim().is_empty();
        if ui
            .add_enabled(
                can_join,
                egui::Button::new(
                    egui::RichText::new("Join")
                        .size(13.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(egui::Color32::from_rgb(40, 140, 80))
                .corner_radius(4.0),
            )
            .clicked()
        {
            let game_id = ctx.competitive_menu.join_game_id.trim().to_string();
            let local_node_id = ctx
                .network_state
                .as_ref()
                .and_then(|ns| {
                    ns.node_id
                        .as_ref()
                        .map(|id| bs58::encode(id.as_bytes()).into_string())
                })
                .unwrap_or_else(|| "unknown".to_string());
            let joiner_display_name = ctx.player_identity.display_name().to_string();
            let joiner_elo_str = ctx.player_identity.display_elo();
            let tx = ctx
                .p2p_vps_state
                .as_ref()
                .map(|vps| vps.response_tx.clone());
            if let Some(tx) = tx {
                let node_id_for_ack = local_node_id.clone();
                std::thread::spawn(move || {
                    match crate::multiplayer::network::vps::p2p_join_game(
                        game_id.clone(),
                        &local_node_id,
                    ) {
                        Ok(Some(host_id)) => {
                            let ack = format!(
                                "JOIN_ACK:{}|{}|{}",
                                node_id_for_ack, joiner_display_name, joiner_elo_str
                            );
                            if let Err(e) = crate::multiplayer::vps_client::p2p_send_message(
                                game_id.clone(),
                                &node_id_for_ack,
                                &ack,
                            ) {
                                warn!("[LOBBY] JOIN_ACK send failed: {}", e);
                            }
                            let _ = tx.send(
                                crate::multiplayer::network::p2p_vps::VpsResponse::JoinResult {
                                    game_id,
                                    host_node_id: Some(host_id),
                                    stake_amount: 0.0,
                                },
                            );
                        }
                        Ok(None) => warn!("[LOBBY] Join by code rejected by VPS"),
                        Err(e) => error!("[LOBBY] Join by code error: {}", e),
                    }
                });
            }
            ctx.competitive_menu.join_game_id.clear();
        }
    });
    ui.add_space(8.0);

    // ── Filter + sort bar ────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Filter:")
                .size(12.0)
                .color(egui::Color32::GRAY),
        );
        for (label, filter) in [
            ("All", crate::states::main_menu::LobbyFilter::All),
            ("Free", crate::states::main_menu::LobbyFilter::Free),
        ] {
            if ui
                .selectable_label(
                    ctx.competitive_menu.lobby_filter == filter,
                    egui::RichText::new(label).size(12.0),
                )
                .clicked()
            {
                ctx.competitive_menu.lobby_filter = filter;
            }
        }

        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("TC:")
                .size(12.0)
                .color(egui::Color32::GRAY),
        );
        for (label, min_opt, max_opt) in [
            ("Any", None, None),
            ("Bullet", None, Some(120u32)),
            ("Blitz", Some(121u32), Some(479u32)),
            ("Rapid+", Some(480u32), None),
        ] {
            let selected = ctx.competitive_menu.lobby_tc_min == min_opt
                && ctx.competitive_menu.lobby_tc_max == max_opt;
            if ui
                .selectable_label(selected, egui::RichText::new(label).size(12.0))
                .clicked()
            {
                ctx.competitive_menu.lobby_tc_min = min_opt;
                ctx.competitive_menu.lobby_tc_max = max_opt;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            use crate::multiplayer::social::LobbySort;
            egui::ComboBox::from_id_salt("lobby_sort")
                .selected_text(
                    egui::RichText::new(match ctx.competitive_menu.lobby_sort {
                        LobbySort::Newest => "Newest",
                        LobbySort::EloAsc => "ELO ↑",
                        LobbySort::EloDesc => "ELO ↓",
                        LobbySort::StakeAsc => "Newest",
                        LobbySort::StakeDesc => "Newest",
                        LobbySort::TimeAsc => "Time ↑",
                    })
                    .size(12.0),
                )
                .show_ui(ui, |ui| {
                    for (label, val) in [
                        ("Newest", LobbySort::Newest),
                        ("ELO ↑", LobbySort::EloAsc),
                        ("ELO ↓", LobbySort::EloDesc),
                        ("Time ↑", LobbySort::TimeAsc),
                    ] {
                        ui.selectable_value(&mut ctx.competitive_menu.lobby_sort, val, label);
                    }
                });
        });
    });
    ui.add_space(4.0);

    ui.heading(
        egui::RichText::new("Open Lobbies")
            .size(20.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(8.0);

    // Build + filter + sort the game list
    let all_games = ctx
        .p2p_vps_state
        .as_ref()
        .map(|v| v.cached_games.clone())
        .unwrap_or_default();
    let last_poll_age = ctx
        .p2p_vps_state
        .as_ref()
        .and_then(|v| v.last_poll.map(|t| t.elapsed().as_secs_f32()));
    let is_fetching = last_poll_age.map(|a| a < 1.5).unwrap_or(false) && all_games.is_empty();

    let filter = ctx.competitive_menu.lobby_filter;
    let tc_min = ctx.competitive_menu.lobby_tc_min;
    let tc_max = ctx.competitive_menu.lobby_tc_max;

    let mut games: Vec<_> = all_games
        .iter()
        .filter(|g| match filter {
            crate::states::main_menu::LobbyFilter::Free => g.stake_amount == 0.0,
            crate::states::main_menu::LobbyFilter::Wagered => g.stake_amount > 0.0,
            crate::states::main_menu::LobbyFilter::All => true,
        })
        .filter(|g| {
            tc_min.map(|m| g.base_time_seconds >= m).unwrap_or(true)
                && tc_max.map(|m| g.base_time_seconds <= m).unwrap_or(true)
        })
        .collect();

    use crate::multiplayer::social::LobbySort;
    match ctx.competitive_menu.lobby_sort {
        LobbySort::EloAsc => games.sort_by(|a, b| a.elo.unwrap_or(0).cmp(&b.elo.unwrap_or(0))),
        LobbySort::EloDesc => games.sort_by(|a, b| b.elo.unwrap_or(0).cmp(&a.elo.unwrap_or(0))),
        LobbySort::StakeAsc => games.sort_by(|a, b| {
            a.stake_amount
                .partial_cmp(&b.stake_amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        LobbySort::StakeDesc => games.sort_by(|a, b| {
            b.stake_amount
                .partial_cmp(&a.stake_amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        LobbySort::TimeAsc => games.sort_by_key(|g| g.base_time_seconds),
        LobbySort::Newest => {}
    }

    if is_fetching {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            ui.spinner();
            ui.label(
                egui::RichText::new("Loading lobbies…")
                    .size(13.0)
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
        });
        return;
    }

    if games.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("No open lobbies right now.")
                    .size(14.0)
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Create one and wait for someone to join.")
                    .size(12.0)
                    .color(egui::Color32::from_rgba_unmultiplied(200, 200, 200, 120)),
            );
        });
        return;
    }

    let region_label = &ctx.backend_region.label;
    let region_latency = ctx.backend_region.latency_ms;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for game in &games {
            let name = game.username.as_deref().unwrap_or(game.display_name.as_str());
            let mins = game.base_time_seconds / 60;
            let inc  = game.increment_seconds;
            let time_str = if inc > 0 {
                format!("{}+{}", mins, inc)
            } else {
                format!("{} min", mins)
            };
            let stake_str = if game.stake_amount > 0.0 {
                format!("{:.3} SOL", game.stake_amount)
            } else {
                "Free".to_string()
            };
            let stake_color = if game.stake_amount > 0.0 {
                egui::Color32::from_rgb(255, 190, 80)
            } else {
                egui::Color32::from_rgb(120, 200, 120)
            };

            // Capacity badge
            let capacity_str = format!("{}/{}", game.players_joined, game.capacity);
            let capacity_color = if game.players_joined >= game.capacity {
                egui::Color32::from_rgb(220, 80, 80)
            } else {
                egui::Color32::from_rgb(120, 200, 120)
            };

            // TTL countdown
            let ttl_str = if game.ttl_seconds > 0 {
                if game.ttl_seconds < 60 { format!("{}s", game.ttl_seconds) }
                else { format!("{}m", game.ttl_seconds / 60) }
            } else {
                String::new()
            };

            // Region with latency indicator
            let game_region = game.region.as_deref().unwrap_or("");
            let is_same_region = !game_region.is_empty()
                && (game_region == region_label.as_str() || game_region.contains(region_label.as_str()));
            let region_str = if game_region.is_empty() {
                String::new()
            } else if is_same_region {
                match region_latency {
                    Some(ms) if ms < 80  => format!("📡 {} {}ms", game_region, ms),
                    Some(ms) if ms < 200 => format!("📡 {} {}ms", game_region, ms),
                    Some(ms)             => format!("🌐 {} {}ms", game_region, ms),
                    None                 => format!("📡 {}", game_region),
                }
            } else {
                format!("🌐 {}", game_region)
            };
            let region_color = if is_same_region {
                match region_latency {
                    Some(ms) if ms < 80  => egui::Color32::from_rgb(80, 200, 120),
                    Some(ms) if ms < 200 => egui::Color32::from_rgb(240, 180, 60),
                    _                    => egui::Color32::from_rgb(120, 180, 220),
                }
            } else {
                egui::Color32::from_rgb(120, 180, 220)
            };

            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 50, 220))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Left: player info
                        ui.vertical(|ui| {
                            // Name row + private badge
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(name).size(14.0).color(egui::Color32::WHITE).strong());
                                if game.is_private {
                                    ui.label(egui::RichText::new("🔒").size(11.0).color(egui::Color32::GRAY));
                                }
                                // Capacity badge
                                ui.label(egui::RichText::new(&capacity_str).size(10.0).color(capacity_color));
                            });
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&time_str).size(11.5).color(egui::Color32::from_rgb(160, 190, 255)));
                                ui.label(egui::RichText::new("·").size(11.5).color(egui::Color32::GRAY));
                                ui.label(egui::RichText::new(&stake_str).size(11.5).color(stake_color));
                                if let Some(elo) = game.elo {
                                    ui.label(egui::RichText::new("·").size(11.5).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("{} ELO", elo)).size(11.5).color(egui::Color32::from_rgb(200, 160, 255)));
                                }
                                if !region_str.is_empty() {
                                    ui.label(egui::RichText::new("·").size(11.5).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(&region_str).size(10.5).color(region_color));
                                }
                                if !ttl_str.is_empty() {
                                    ui.label(egui::RichText::new("·").size(11.5).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("expires {}", ttl_str)).size(10.5).color(egui::Color32::from_rgb(180, 140, 80)));
                                }
                            });
                        });

                        // Right: join button
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let full = game.players_joined >= game.capacity;
                            if ui.add_enabled(!full, egui::Button::new(
                                egui::RichText::new(if full { "Full" } else { "Join" }).size(13.0).color(egui::Color32::WHITE).strong()
                            ).fill(if full { egui::Color32::from_rgb(60, 60, 60) } else { egui::Color32::from_rgb(40, 140, 80) }).corner_radius(4.0).min_size(egui::vec2(70.0, 32.0))).clicked() {
                                info!("[LOBBY] Joining: {}", game.game_id);
                                let game_id = game.game_id.clone();
                                let stake_amount = game.stake_amount;
                                let local_node_id = ctx.network_state.as_ref()
                                    .and_then(|ns| ns.node_id.as_ref().map(|id| bs58::encode(id.as_bytes()).into_string()))
                                    .unwrap_or_else(|| "unknown".to_string());
                                let joiner_display_name = ctx.player_identity.display_name().to_string();
                                let joiner_elo_str = ctx.player_identity.display_elo();
                                let tx = ctx.p2p_vps_state.as_ref().map(|vps| vps.response_tx.clone());

                                if let Some(tx) = tx {
                                    std::thread::spawn(move || {
                                        match crate::multiplayer::network::vps::p2p_join_game(game_id.clone(), &local_node_id) {
                                            Ok(Some(host_id)) => {
                                                let ack = format!("JOIN_ACK:{}|{}|{}", local_node_id, joiner_display_name, joiner_elo_str);
                                                if let Err(e) = crate::multiplayer::vps_client::p2p_send_message(game_id.clone(), &local_node_id, &ack) {
                                                    warn!("[LOBBY] JOIN_ACK send failed: {}", e);
                                                } else {
                                                    info!("[LOBBY] Sent JOIN_ACK for {}", game_id);
                                                }
                                                let _ = tx.send(crate::multiplayer::network::p2p_vps::VpsResponse::JoinResult {
                                                    game_id,
                                                    host_node_id: Some(host_id),
                                                    stake_amount,
                                                });
                                            }
                                            Ok(None) => warn!("[LOBBY] Join rejected by VPS"),
                                            Err(e) => error!("[LOBBY] Join error: {}", e),
                                        }
                                    });
                                }
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }
    });
}

/// Part 4B — Tournament browser screen shown when MenuState::Tournaments is active.
pub(super) fn render_tournament_browser_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ctx.learn_viewport.rect_px = None;
    ui.vertical_centered(|ui| {
        ui.heading(egui::RichText::new("TOURNAMENTS").size(24.0).color(egui::Color32::from_rgb(255, 200, 50)).strong());
        ui.add_space(8.0);

        if ui.button("? Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::Main);
        }

        ui.add_space(16.0);

        #[cfg(feature = "solana")]
        {
            let wallet_pubkey = ctx.solana_state.as_ref()
                .and_then(|s| s.wallet_pubkey.map(|p| p.to_string()));

            let is_refreshing = ctx.tournament_client.as_ref()
                .map(|tc| tc.list_rx.is_some())
                .unwrap_or(false);
            let last_error = ctx.tournament_client.as_ref()
                .and_then(|tc| tc.last_poll_error.clone());

            let last_poll_age = ctx.tournament_client.as_ref()
                .and_then(|tc| tc.last_list_poll.map(|t| t.elapsed().as_secs()));

            // Status bar: refresh indicator + last-updated + refresh button + error
            ui.horizontal(|ui| {
                if is_refreshing {
                    ui.label(egui::RichText::new("Refreshing...").size(11.0).color(egui::Color32::from_rgb(100, 200, 255)).italics());
                } else if let Some(secs) = last_poll_age {
                    ui.label(egui::RichText::new(format!("Updated {}s ago", secs)).size(11.0).color(egui::Color32::from_gray(140)).italics());
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !is_refreshing {
                        if ui.add(egui::Button::new(
                            egui::RichText::new("⟳ Refresh").size(11.0)
                        ).fill(egui::Color32::from_rgba_unmultiplied(40, 60, 90, 200))).clicked() {
                            if let Some(ref mut tc) = ctx.tournament_client {
                                tc.last_list_poll = None;
                            }
                        }
                    }
                    if let Some(ref err) = last_error {
                        ui.label(egui::RichText::new(format!("Failed: {}", err)).size(11.0).color(egui::Color32::from_rgb(230, 80, 80)));
                    }
                });
            });
            ui.add_space(4.0);

            // Status filter chips
            let current_filter = ctx.tournament_client.as_ref()
                .and_then(|tc| tc.status_filter.clone());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (label, value) in &[
                    ("All",          None::<&str>),
                    ("Open",         Some("registration")),
                    ("Active",       Some("active")),
                    ("Completed",    Some("completed")),
                ] {
                    let chip_val = value.map(|s| s.to_string());
                    let selected = current_filter == chip_val;
                    let fill = if selected {
                        egui::Color32::from_rgb(60, 120, 200)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(60, 60, 80, 180)
                    };
                    let stroke = if selected {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 160, 255))
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_gray(80))
                    };
                    if ui.add(
                        egui::Button::new(egui::RichText::new(*label).size(11.0).color(egui::Color32::WHITE))
                            .fill(fill)
                            .stroke(stroke)
                            .corner_radius(10.0)
                            .min_size(egui::vec2(58.0, 22.0)),
                    ).clicked() {
                        if let Some(ref mut tc) = ctx.tournament_client {
                            tc.status_filter = chip_val.clone();
                        }
                    }
                }
            });
            ui.add_space(8.0);

            let tournaments = ctx.tournament_client.as_ref()
                .map(|tc| tc.available_tournaments.clone())
                .unwrap_or_else(Vec::new);

            // Filter list by selected status
            let filter_key = ctx.tournament_client.as_ref()
                .and_then(|tc| tc.status_filter.clone());
            let visible_tournaments: Vec<_> = tournaments.iter()
                .filter(|t| {
                    match &filter_key {
                        None => true,
                        Some(k) => t.status.to_lowercase().contains(k.as_str()),
                    }
                })
                .collect();

            // Waiting-for-next-match panel (shown when in an active tournament)
            if let Some(ref tc) = ctx.tournament_client {
                if tc.active_tournament_id.is_some() && tc.waiting_for_next_match {
                    ui.group(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("Tournament Match Complete").size(16.0).color(egui::Color32::WHITE).strong());
                            ui.add_space(6.0);
                            if let Some(ref result) = tc.last_match_result {
                                ui.label(egui::RichText::new(format!("Result: {}", result)).size(14.0).color(egui::Color32::GOLD).strong());
                            }
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Waiting for next opponent…").size(13.0).color(egui::Color32::from_rgb(100, 200, 255)).italics());
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("The next round will begin automatically when all matches finish.").size(11.0).color(egui::Color32::GRAY));
                        });
                    });
                    ui.add_space(12.0);
                }
            }

            if visible_tournaments.is_empty() {
                if tournaments.is_empty() {
                    ui.label(egui::RichText::new("No tournaments available. Check back later.").size(13.0).color(egui::Color32::GRAY).italics());
                } else {
                    ui.label(egui::RichText::new("No tournaments match the current filter.").size(13.0).color(egui::Color32::GRAY).italics());
                }
            } else {
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    for t in &visible_tournaments {
                        let is_password_prompt = ctx.tournament_client.as_ref()
                            .map(|tc| tc.active_tournament_id == Some(t.tournament_id) && t.is_private)
                            .unwrap_or(false);
                        let is_expanded = ctx.tournament_client.as_ref()
                            .map(|tc| tc.expanded_tournament_id == Some(t.tournament_id))
                            .unwrap_or(false);

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    // Header row: name + format badge
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(&t.name).size(14.0).color(egui::Color32::WHITE).strong());
                                        let (badge_text, badge_color) = if t.is_tournament {
                                            ("Swiss", egui::Color32::from_rgb(255, 200, 50))
                                        } else {
                                            ("Game", egui::Color32::from_rgb(100, 200, 180))
                                        };
                                        ui.add_space(4.0);
                                        ui.add(egui::Button::new(
                                            egui::RichText::new(badge_text).size(9.0).color(egui::Color32::BLACK).strong()
                                        ).fill(badge_color).corner_radius(8.0)
                                         .min_size(egui::vec2(0.0, 16.0))
                                         .sense(egui::Sense::hover()));
                                        // Status chip
                                        let (status_label, status_col) = match t.status.to_lowercase().as_str() {
                                            s if s.contains("registration") => ("● Registration", egui::Color32::from_rgb(60, 200, 100)),
                                            s if s.contains("active")       => ("● Active",       egui::Color32::from_rgb(255, 180, 0)),
                                            s if s.contains("completed")    => ("● Completed",    egui::Color32::from_gray(140)),
                                            s if s.contains("cancelled")    => ("✕ Cancelled",    egui::Color32::from_rgb(220, 80, 80)),
                                            _                               => ("● Unknown",      egui::Color32::GRAY),
                                        };
                                        ui.add_space(6.0);
                                        ui.label(egui::RichText::new(status_label).size(10.0).color(status_col).strong());
                                    });
                                    // Entry fee + prize pool row
                                    ui.horizontal(|ui| {
                                        let entry = if t.entry_fee_lamports == 0 {
                                            "Free".to_string()
                                        } else {
                                            format!("{:.3} SOL entry", t.entry_fee_lamports as f64 / 1e9)
                                        };
                                        ui.label(egui::RichText::new(entry).size(11.0).color(egui::Color32::from_rgb(150, 200, 150)));
                                        if t.prize_pool > 0 {
                                            ui.add_space(8.0);
                                            let prize_str = if t.usdc_mint.is_some() {
                                                format!("Prize: {:.2} USDC", t.prize_pool as f64 / 1_000_000.0)
                                            } else {
                                                format!("Prize: {:.3} SOL", t.prize_pool as f64 / 1e9)
                                            };
                                            ui.label(egui::RichText::new(prize_str)
                                                .size(11.0).color(egui::Color32::GOLD));
                                        }
                                    });
                                    // Registered count
                                    let lock_icon = if t.is_private { "  " } else { "" };
                                    ui.label(egui::RichText::new(
                                        format!("{}{}  registered", lock_icon, t.registered)
                                    ).size(10.0).color(egui::Color32::from_gray(160)));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let can_join = wallet_pubkey.is_some();
                                    let join_btn = ui.add_enabled(can_join && !is_password_prompt, egui::Button::new(
                                        egui::RichText::new("Join").size(12.0).color(egui::Color32::WHITE).strong()
                                    ).fill(if can_join { egui::Color32::from_rgb(50, 140, 50) } else { egui::Color32::from_rgb(60, 60, 60) }).corner_radius(4.0).min_size(egui::vec2(60.0, 28.0)));
                                    if t.is_private {
                                        ui.label(egui::RichText::new("").size(14.0).color(egui::Color32::WHITE));
                                    }
                                    // Expand/collapse toggle
                                    let expand_icon = if is_expanded { "▲" } else { "▼" };
                                    if ui.add(
                                        egui::Button::new(egui::RichText::new(expand_icon).size(11.0).color(egui::Color32::from_gray(180)))
                                            .fill(egui::Color32::TRANSPARENT)
                                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(70)))
                                            .min_size(egui::vec2(24.0, 26.0)),
                                    ).clicked() {
                                        let tid = t.tournament_id;
                                        if let Some(ref mut tc) = ctx.tournament_client {
                                            if tc.expanded_tournament_id == Some(tid) {
                                                tc.expanded_tournament_id = None;
                                            } else {
                                                tc.expanded_tournament_id = Some(tid);
                                            }
                                        }
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
                                                std::thread::spawn(move || {
                                                    match crate::multiplayer::network::vps::join_tournament(tid, &pk, None) {
                                                        Ok(slot) => info!("[TOURNAMENT] Joined tournament {} slot {}", tid, slot),
                                                        Err(e) => warn!("[TOURNAMENT] Join failed: {}", e),
                                                    }
                                                });
                                                tc.join_status = crate::multiplayer::solana::tournament::TournamentJoinStatus::Pending;
                                            }
                                        }
                                    }
                                });
                            });

                            // Expanded details panel
                            if is_expanded {
                                ui.separator();
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 16.0;
                                    if t.prize_pool > 0 {
                                        let pool_str = if t.usdc_mint.is_some() {
                                            format!("Prize pool: {:.2} USDC", t.prize_pool as f64 / 1_000_000.0)
                                        } else {
                                            format!("Prize pool: {:.3} SOL", t.prize_pool as f64 / 1e9)
                                        };
                                        ui.label(egui::RichText::new(pool_str).size(11.0).color(egui::Color32::GOLD));
                                    }
                                    let kind = if t.is_tournament { "Swiss/SE Tournament" } else if t.entry_fee_lamports > 0 { "Wagered 1v1" } else { "Casual 1v1" };
                                    ui.label(egui::RichText::new(format!("Format: {}", kind)).size(11.0).color(egui::Color32::from_gray(190)));
                                    let status_icon = match t.status.to_lowercase().as_str() {
                                        s if s.contains("registration") => "  Open for registration",
                                        s if s.contains("active") => "  In progress",
                                        s if s.contains("completed") => "  Finished",
                                        _ => "  Unknown status",
                                    };
                                    ui.label(egui::RichText::new(status_icon).size(11.0).color(egui::Color32::from_rgb(100, 200, 255)));
                                    if t.max_players > 0 {
                                        ui.label(egui::RichText::new(format!("{}/{} players", t.registered, t.max_players)).size(11.0).color(egui::Color32::from_gray(160)));
                                    }
                                });

                                // ELO requirement display
                                if t.min_elo > 0 || t.max_elo > 0 {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        ui.label(egui::RichText::new("ELO:").size(11.0).color(egui::Color32::from_gray(140)));
                                        if t.min_elo > 0 && t.max_elo > 0 {
                                            ui.label(egui::RichText::new(format!("{} – {}", t.min_elo, t.max_elo)).size(11.0).color(egui::Color32::from_rgb(200, 180, 100)));
                                        } else if t.min_elo > 0 {
                                            ui.label(egui::RichText::new(format!("{}+", t.min_elo)).size(11.0).color(egui::Color32::from_rgb(200, 180, 100)));
                                        } else {
                                            ui.label(egui::RichText::new(format!("≤ {}", t.max_elo)).size(11.0).color(egui::Color32::from_rgb(200, 180, 100)));
                                        }
                                    });
                                }

                                // Round timer chip — shown for active tournaments with a deadline
                                if t.status.to_lowercase().contains("active") {
                                    if let Some(deadline) = t.round_deadline_at {
                                        let now_secs = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .map(|d| d.as_secs() as i64)
                                            .unwrap_or(0);
                                        if deadline > now_secs {
                                            let remaining = (deadline - now_secs) as u64;
                                            let mins = remaining / 60;
                                            let secs = remaining % 60;
                                            let chip_color = if remaining < 60 {
                                                egui::Color32::from_rgb(220, 60, 60)
                                            } else if remaining < 300 {
                                                egui::Color32::from_rgb(220, 160, 40)
                                            } else {
                                                egui::Color32::from_gray(180)
                                            };
                                            ui.label(egui::RichText::new(
                                                format!("Round ends in {}m {:02}s", mins, secs)
                                            ).size(11.0).color(chip_color));
                                        }
                                    }
                                }

                                // Watch button stub — shown for active tournaments
                                if t.status.to_lowercase().contains("active") {
                                    ui.add_space(6.0);
                                    let watch_btn = ui.add_enabled(
                                        false,
                                        egui::Button::new(
                                            egui::RichText::new("  Watch Live").size(12.0).color(egui::Color32::from_gray(120)).strong()
                                        )
                                        .fill(egui::Color32::from_rgb(40, 40, 60))
                                        .corner_radius(5.0)
                                        .min_size(egui::vec2(120.0, 28.0)),
                                    );
                                    watch_btn.on_disabled_hover_text("Live spectating — coming soon");
                                }

                                // Register button — only shown when status is "registration"
                                if t.status.to_lowercase().contains("registration") {
                                    ui.add_space(6.0);
                                    let my_elo = ctx.solana_state.as_ref().map(|s| s.cached_elo).unwrap_or(0);
                                    let balance = ctx.solana_state.as_ref().map(|s| s.balance).unwrap_or(0.0);
                                    let entry_sol = t.entry_fee_lamports as f64 / 1e9;
                                    let my_elo_u32 = my_elo as u32;
                                    let elo_ok = (t.min_elo == 0 || my_elo_u32 >= t.min_elo)
                                        && (t.max_elo == 0 || my_elo_u32 <= t.max_elo || my_elo == 0);
                                    let can_afford = t.entry_fee_lamports == 0 || balance >= entry_sol + 0.01;
                                    let has_wallet = wallet_pubkey.is_some();
                                    let can_register = has_wallet && elo_ok && can_afford && !is_password_prompt;

                                    let tooltip = if !has_wallet {
                                        Some("Connect wallet to register")
                                    } else if !can_afford {
                                        Some("Insufficient balance")
                                    } else if !elo_ok {
                                        Some("ELO does not meet requirements")
                                    } else {
                                        None
                                    };

                                    ui.horizontal(|ui| {
                                        let btn = ui.add_enabled(
                                            can_register,
                                            egui::Button::new(
                                                egui::RichText::new("  Register").size(13.0).color(egui::Color32::WHITE).strong()
                                            )
                                            .fill(if can_register { egui::Color32::from_rgb(60, 150, 60) } else { egui::Color32::from_rgb(60, 60, 60) })
                                            .corner_radius(5.0)
                                            .min_size(egui::vec2(120.0, 30.0)),
                                        );
                                        let btn = if let Some(tip) = tooltip { btn.on_hover_text(tip) } else { btn };
                                        if btn.clicked() {
                                            let tid = t.tournament_id;
                                            if let Some(ref mut tc) = ctx.tournament_client {
                                                tc.active_tournament_id = Some(tid);
                                                if t.is_private {
                                                    tc.password_input = String::new();
                                                    tc.password_error = None;
                                                } else {
                                                    let pk = wallet_pubkey.clone().unwrap_or_default();
                                                    std::thread::spawn(move || {
                                                        match crate::multiplayer::network::vps::join_tournament(tid, &pk, None) {
                                                            Ok(slot) => info!("[TOURNAMENT] Registered for {} slot {}", tid, slot),
                                                            Err(e) => warn!("[TOURNAMENT] Register failed: {}", e),
                                                        }
                                                    });
                                                    tc.join_status = crate::multiplayer::solana::tournament::TournamentJoinStatus::Pending;
                                                }
                                            }
                                        }
                                        if t.entry_fee_lamports > 0 && has_wallet {
                                            let bal_col = if can_afford { egui::Color32::from_gray(130) } else { egui::Color32::from_rgb(220, 80, 80) };
                                            ui.label(egui::RichText::new(format!("Cost: {:.3} SOL  (bal: {:.3})", entry_sol, balance)).size(10.0).color(bal_col));
                                        }
                                    });
                                }
                                ui.add_space(4.0);
                            }

                            // Waiting room — shown after registration
                            let is_waiting = ctx.tournament_client.as_ref()
                                .map(|tc| tc.is_registered() && tc.active_tournament_id == Some(t.tournament_id))
                                .unwrap_or(false);
                            if is_waiting {
                                if let Some(ref mut tc) = ctx.tournament_client {
                                    ui.add_space(6.0);
                                    ui.separator();
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new("WAITING ROOM").size(11.0).strong()
                                        .color(egui::Color32::from_rgb(100, 200, 255)));
                                    ui.add_space(4.0);

                                    // Fill progress bar
                                    if t.max_players > 0 {
                                        let fill = tc.registered_players.len().min(t.max_players as usize) as f32
                                            / t.max_players as f32;
                                        let bar = egui::widgets::ProgressBar::new(fill)
                                            .text(format!("{}/{} players", tc.registered_players.len(), t.max_players));
                                        ui.add(bar);
                                        ui.add_space(4.0);
                                    }

                                    // Registered player list
                                    let slots_to_show = t.max_players.min(8) as usize;
                                    for idx in 0..slots_to_show {
                                        let label = tc.registered_players.get(idx)
                                            .map(|n| n.as_str())
                                            .unwrap_or("—");
                                        let col = if idx < tc.registered_players.len() {
                                            egui::Color32::from_rgb(200, 220, 200)
                                        } else {
                                            egui::Color32::from_gray(90)
                                        };
                                        ui.label(egui::RichText::new(
                                            format!("{}. {}", idx + 1, label)
                                        ).size(10.0).color(col));
                                    }
                                    ui.add_space(6.0);

                                    // Tournament waiting-room chat
                                    tc.drain_chat();
                                    if tc.chat_rx.is_none() {
                                        let player = ctx.player_identity.display_name().to_string();
                                        tc.start_chat(t.tournament_id, player);
                                    }
                                    ui.separator();
                                    ui.label(egui::RichText::new("CHAT").size(10.0).strong()
                                        .color(egui::Color32::from_rgb(180, 180, 220)));
                                    egui::ScrollArea::vertical()
                                        .id_salt("tournament_chat_scroll")
                                        .max_height(80.0)
                                        .stick_to_bottom(true)
                                        .show(ui, |ui| {
                                            for (sender, text) in &tc.chat_messages {
                                                ui.horizontal_wrapped(|ui| {
                                                    ui.label(egui::RichText::new(format!("{}:", sender)).size(10.0)
                                                        .color(egui::Color32::from_rgb(150, 200, 255)).strong());
                                                    ui.label(egui::RichText::new(text).size(10.0)
                                                        .color(egui::Color32::WHITE));
                                                });
                                            }
                                        });
                                    ui.horizontal(|ui| {
                                        ui.add(egui::TextEdit::singleline(&mut tc.chat_input)
                                            .desired_width(100.0).hint_text("Message…").font(egui::TextStyle::Small));
                                        let can_send = !tc.chat_input.trim().is_empty();
                                        if ui.add_enabled(can_send, egui::Button::new(
                                            egui::RichText::new("Send").size(10.0)
                                        ).fill(egui::Color32::from_rgb(40, 100, 60))).clicked() {
                                            let text = tc.chat_input.trim().to_string();
                                            let player = ctx.player_identity.display_name().to_string();
                                            tc.chat_messages.push((player.clone(), text.clone()));
                                            if let Some(ref tx) = tc.chat_tx {
                                                let _ = tx.send((player, text));
                                            }
                                            tc.chat_input.clear();
                                        }
                                    });
                                    ui.add_space(4.0);

                                    // Leave button
                                    if ui.add(egui::Button::new(
                                        egui::RichText::new("Leave").size(11.0).color(egui::Color32::WHITE)
                                    ).fill(egui::Color32::from_rgb(100, 40, 40))).clicked() {
                                        let tid = t.tournament_id;
                                        let base = crate::multiplayer::network::vps::vps_base();
                                        std::thread::spawn(move || {
                                            let url = format!("{}/api/tournament/{}/leave", base, tid);
                                            let _ = reqwest::blocking::Client::new().post(&url).send();
                                        });
                                        tc.join_status = crate::multiplayer::solana::tournament::TournamentJoinStatus::Idle;
                                        tc.active_tournament_id = None;
                                        tc.registered_players.clear();
                                    }
                                }
                            }

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
                                            std::thread::spawn(move || {
                                                match crate::multiplayer::network::vps::join_tournament(tid, &pk, Some(&password)) {
                                                    Ok(slot) => info!("[TOURNAMENT] Joined private tournament {} slot {}", tid, slot),
                                                    Err(e) => warn!("[TOURNAMENT] Private join failed: {}", e),
                                                }
                                            });
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
        // Private join code field
        #[cfg(feature = "solana")]
        if let Some(ref mut tc) = ctx.tournament_client {
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Join with code:").size(12.0).color(egui::Color32::from_gray(180)));
                ui.add(egui::TextEdit::singleline(&mut tc.private_code_input)
                    .hint_text("Invite code")
                    .desired_width(120.0));
                let can_submit = !tc.private_code_input.is_empty();
                if ui.add_enabled(can_submit, egui::Button::new(
                    egui::RichText::new("Join").size(12.0)
                ).fill(egui::Color32::from_rgb(60, 100, 160))).clicked() {
                    let code = tc.private_code_input.clone();
                    let base = crate::multiplayer::network::vps::vps_base();
                    let (tx, rx) = crossbeam_channel::bounded(1);
                    tc.private_join_rx = Some(rx);
                    tc.private_code_error = None;
                    std::thread::spawn(move || {
                        let url = format!("{}/api/tournament/join-private/{}", base, code);
                        let result = reqwest::blocking::Client::new().post(&url).send();
                        let _ = tx.send(match result {
                            Ok(r) if r.status().is_success() => Ok(()),
                            Ok(r) => Err(format!("Error {}", r.status())),
                            Err(e) => Err(e.to_string()),
                        });
                    });
                }
            });
            // Drain private join result
            if let Some(ref rx) = tc.private_join_rx {
                if let Ok(result) = rx.try_recv() {
                    match result {
                        Ok(()) => {
                            tc.join_status = crate::multiplayer::solana::tournament::TournamentJoinStatus::Pending;
                            tc.private_code_input.clear();
                            tc.private_code_error = None;
                        }
                        Err(e) => { tc.private_code_error = Some(e); }
                    }
                    tc.private_join_rx = None;
                }
            }
            if let Some(ref err) = tc.private_code_error.clone() {
                ui.label(egui::RichText::new(err).size(11.0).color(egui::Color32::from_rgb(255, 100, 100)));
            }
        }

        #[cfg(not(feature = "solana"))]
        ui.label(egui::RichText::new("Tournament browser requires the solana feature.").size(13.0).color(egui::Color32::GRAY).italics());
    });
}

pub(super) fn render_host_p2p_config_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    // (The welcome card moved to the startup main menu — see render_welcome_panel.)
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("Create Game")
                .size(24.0)
                .color(egui::Color32::from_rgb(100, 200, 255))
                .strong(),
        );
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.label(
                egui::RichText::new("Room Name")
                    .size(15.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut ctx.p2p_host.lobby_name)
                    .hint_text("Leave blank for your username")
                    .desired_width(f32::INFINITY),
            );
        });

        ui.add_space(12.0);

        ui.group(|ui| {
            ui.label(
                egui::RichText::new("Time Control")
                    .size(15.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.add_space(8.0);

            // Preset mode buttons
            ui.horizontal_wrapped(|ui| {
                for (label, base_min, inc_sec) in [
                    ("Bullet 1+0", 1u32, 0u16),
                    ("Blitz 3+2", 3, 2),
                    ("Blitz 5+0", 5, 0),
                    ("Rapid 10+0", 10, 0),
                    ("Rapid 15+10", 15, 10),
                    ("30 min", 30, 0),
                ] {
                    let selected = ctx.p2p_host.base_time_minutes == base_min
                        && ctx.p2p_host.increment_seconds == inc_sec;
                    let btn = egui::Button::new(egui::RichText::new(label).size(13.0)).fill(
                        if selected {
                            egui::Color32::from_rgb(40, 120, 200)
                        } else {
                            egui::Color32::from_rgb(40, 40, 50)
                        },
                    );
                    if ui.add(btn).clicked() {
                        ctx.p2p_host.base_time_minutes = base_min;
                        ctx.p2p_host.increment_seconds = inc_sec;
                    }
                }
            });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Custom")
                    .size(12.0)
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(2.0);

            ui.horizontal(|ui| {
                ui.label("Minutes per side:");
                ui.add(egui::Slider::new(
                    &mut ctx.p2p_host.base_time_minutes,
                    1..=60,
                ));
            });
            ui.horizontal(|ui| {
                ui.label("Increment (seconds):");
                ui.add(egui::Slider::new(
                    &mut ctx.p2p_host.increment_seconds,
                    0..=60,
                ));
            });
        });

        ui.add_space(24.0);

        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new("Cancel").size(16.0))
                .clicked()
            {
                ctx.menu_state.set(crate::core::MenuState::Main);
            }

            ui.add_space(12.0);

            let node_id_ready = ctx
                .network_state
                .as_ref()
                .map(|ns| ns.node_id.is_some())
                .unwrap_or(false);

            let start_btn = ui.add_enabled(
                node_id_ready,
                egui::Button::new(
                    egui::RichText::new(" Start Hosting")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(egui::Color32::from_rgb(40, 140, 80)),
            );

            if start_btn.clicked() {
                let game_id = format!("p2p_{}", rand::random::<u32>());
                ctx.p2p_host.game_id = Some(game_id.clone());
                ctx.p2p_host.last_heartbeat = Some(std::time::Instant::now());

                // Firing events for internal systems
                if let Some(host_events) = &mut ctx.host_game_events {
                    host_events.write(crate::multiplayer::network::p2p::HostGameEvent);
                }

                // Announce to VPS (off render thread — blocking HTTP with 120s timeout)
                let display_name = if ctx.p2p_host.lobby_name.trim().is_empty() {
                    ctx.player_identity.display_name().to_string()
                } else {
                    ctx.p2p_host.lobby_name.trim().to_string()
                };
                let host_node_id = ctx
                    .network_state
                    .as_ref()
                    .and_then(|ns| {
                        ns.node_id
                            .map(|id| bs58::encode(id.as_bytes()).into_string())
                    })
                    .unwrap_or_default();

                // Register with the host-side relay poller so we detect joiners via HTTP
                if let Some(ref mut vps) = ctx.p2p_vps_state {
                    vps.hosting_game_id = Some(game_id.clone());
                    vps.hosting_node_id = Some(host_node_id.clone());
                    vps.host_poll_last = None; // trigger immediately
                    vps.hosting_stake_amount = 0.0;
                    vps.hosting_base_secs = (ctx.p2p_host.base_time_minutes * 60) as u32;
                    vps.hosting_inc = ctx.p2p_host.increment_seconds;
                    vps.pending_joiner = None; // clear any stale joiner from previous session
                }
                {
                    let gid = game_id.clone();
                    let nid = host_node_id.clone();
                    let dn = display_name.clone();
                    let stake = 0.0_f64;
                    let base_secs = (ctx.p2p_host.base_time_minutes * 60) as u32;
                    let inc = ctx.p2p_host.increment_seconds as u16;
                    std::thread::spawn(move || {
                        match crate::multiplayer::vps_client::p2p_announce_game(
                            gid,
                            &nid,
                            &dn,
                            stake,
                            "P2P",
                            base_secs,
                            inc,
                            Some(dn.clone()),
                            None,
                            None,
                        ) {
                            Ok(()) => info!("[LOBBY] P2P game announced to VPS relay"),
                            Err(e) => warn!("[LOBBY] P2P announce failed: {}", e),
                        }
                    });
                }

                info!(
                    "[LOBBY] Hosting P2P game: {} ({} + {})",
                    game_id, ctx.p2p_host.base_time_minutes, ctx.p2p_host.increment_seconds
                );

                // Transition to Waiting Screen
                ctx.menu_state.set(crate::core::MenuState::P2PWaiting);

                // Also update internal connection state
                if let Some(ref mut p2p_state) = ctx.p2p_state {
                    p2p_state.status = P2PConnectionStatus::Hosting;
                }
            }

            if !node_id_ready {
                ui.label(
                    egui::RichText::new("Wait for P2P initialization…")
                        .size(11.0)
                        .color(egui::Color32::RED),
                );
            }
        });
    });
}

pub(super) fn render_p2p_waiting_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    // Heartbeat: keep the lobby alive on the backend every 60 seconds.
    let should_heartbeat = ctx
        .p2p_host
        .last_heartbeat
        .map(|t| t.elapsed().as_secs() >= 60)
        .unwrap_or(false); // announce just happened, first heartbeat at 60s
    if should_heartbeat {
        if let Some(game_id) = ctx.p2p_host.game_id.clone() {
            let node_id = ctx
                .network_state
                .as_ref()
                .and_then(|ns| {
                    ns.node_id
                        .map(|id| bs58::encode(id.as_bytes()).into_string())
                })
                .unwrap_or_default();
            ctx.p2p_host.last_heartbeat = Some(std::time::Instant::now());
            std::thread::spawn(move || {
                if let Err(e) = crate::multiplayer::vps_client::p2p_heartbeat(game_id, &node_id) {
                    warn!("[LOBBY] Heartbeat failed: {}", e);
                }
            });
        }
    }

    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.heading(
            egui::RichText::new("WAITING FOR OPPONENT")
                .size(24.0)
                .color(egui::Color32::GOLD)
                .strong(),
        );
        ui.add_space(20.0);

        ui.label(
            egui::RichText::new("Your game is now visible in the lobby.")
                .size(14.0)
                .color(egui::Color32::WHITE),
        );
        ui.add_space(8.0);

        if let Some(game_id) = ctx.p2p_host.game_id.clone() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Code: {}", game_id))
                        .size(12.0)
                        .color(egui::Color32::GRAY)
                        .monospace(),
                );
                if ui
                    .small_button("Copy")
                    .on_hover_text("Copy game code to clipboard")
                    .clicked()
                {
                    ui.output_mut(|o| {
                        o.commands
                            .push(egui::OutputCommand::CopyText(game_id.clone()))
                    });
                }
            });
        }
        ui.add_space(30.0);

        // Show joiner identity once detected, otherwise animated dots
        let pending_joiner = ctx
            .p2p_vps_state
            .as_ref()
            .and_then(|vps| vps.pending_joiner.clone());
        if let Some(ref joiner) = pending_joiner {
            ui.label(
                egui::RichText::new("Opponent found!")
                    .size(16.0)
                    .color(egui::Color32::from_rgb(100, 220, 100))
                    .strong(),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(&joiner.display_name)
                    .size(22.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            if joiner.elo_str != "—" {
                ui.label(
                    egui::RichText::new(format!("ELO  {}", joiner.elo_str))
                        .size(14.0)
                        .color(egui::Color32::GOLD),
                );
            }
        } else {
            ui.label(
                egui::RichText::new("• • •")
                    .size(32.0)
                    .color(egui::Color32::GOLD),
            );
        }

        ui.add_space(40.0);

        // Friends invite panel
        {
            let game_id = ctx.p2p_host.game_id.clone().unwrap_or_default();
            let host_node_id = ctx
                .network_state
                .as_ref()
                .and_then(|ns| {
                    ns.node_id
                        .map(|id| bs58::encode(id.as_bytes()).into_string())
                })
                .unwrap_or_default();
            let online_contacts: Vec<_> = ctx
                .friends
                .contacts
                .iter()
                .filter(|c| c.is_online)
                .cloned()
                .collect();
            if !online_contacts.is_empty() {
                ui.separator();
                ui.label(
                    egui::RichText::new("Invite a Friend")
                        .size(13.0)
                        .color(egui::Color32::LIGHT_BLUE),
                );
                ui.add_space(4.0);
                for contact in online_contacts {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&contact.contact_display)
                                .size(12.0)
                                .color(egui::Color32::WHITE),
                        );
                        if ui.small_button("Invite").clicked()
                            && !game_id.is_empty()
                            && !host_node_id.is_empty()
                        {
                            let gid = game_id.clone();
                            let nid = host_node_id.clone();
                            let from_display = ctx.friends.our_display.clone();
                            let to_node = contact.contact_node_id.clone();
                            std::thread::spawn(move || {
                                if let Err(e) = crate::multiplayer::vps_client::push_lobby_invite(
                                    &gid,
                                    &nid,
                                    &from_display,
                                    &to_node,
                                ) {
                                    warn!(
                                        "[INVITE] Failed to send lobby invite to {}: {}",
                                        to_node, e
                                    );
                                } else {
                                    info!("[INVITE] Sent lobby invite to {}", to_node);
                                }
                            });
                        }
                    });
                }
                ui.add_space(12.0);
            }
        }

        if ui
            .button(
                egui::RichText::new(" Cancel Hosting")
                    .size(16.0)
                    .color(egui::Color32::from_rgb(255, 100, 100)),
            )
            .clicked()
        {
            if let Some(game_id) = ctx.p2p_host.game_id.clone() {
                let node_id = ctx
                    .network_state
                    .as_ref()
                    .and_then(|ns| {
                        ns.node_id
                            .map(|id| bs58::encode(id.as_bytes()).into_string())
                    })
                    .unwrap_or_default();
                // Non-blocking — don't freeze the render thread on cancel
                std::thread::spawn(move || {
                    if let Err(e) =
                        crate::multiplayer::vps_client::p2p_leave_game(game_id.clone(), &node_id)
                    {
                        warn!("[LOBBY] Leave failed on cancel: {}", e);
                    } else {
                        info!("[LOBBY] Cancelled hosting for {}", game_id);
                    }
                });
            }

            ctx.p2p_host.game_id = None;
            ctx.p2p_host.last_heartbeat = None;
            if let Some(ref mut p2p_state) = ctx.p2p_state {
                p2p_state.status = P2PConnectionStatus::Disconnected;
            }
            // Stop host relay polling
            if let Some(ref mut vps) = ctx.p2p_vps_state {
                vps.hosting_game_id = None;
                vps.hosting_node_id = None;
                vps.pending_joiner = None;
            }
            ctx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
    });
}
