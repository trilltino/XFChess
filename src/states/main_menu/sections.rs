//! Content sections rendered inside the website-style main menu.
//!
//! This module owns the individual cards that sit under the navbar: PLAY,
//! QUICK PLAY, NEWS, LEARN (which hosts the 3D viewport used by
//! `XfAnimatePlugin`), TOURNAMENTS, UPDATES, and the legacy LOBBY list. Each
//! helper takes only the state it needs so the orchestrator in `main_menu.rs`
//! stays small and composable.

use super::*;
use crate::core::GameState;
use bevy_egui::egui;
use tracing::{debug, info, warn};

/// Render the "PLAY" card: create/join lobby + VS Computer shortcut.
pub(super) fn render_play_computer_section(ui: &mut egui::Ui, ctx_menu: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("PLAY")
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );
    });
    ui.add_space(15.0);

    let play_button = |text: &str| {
        egui::Button::new(
            egui::RichText::new(text)
                .size(15.0)
                .color(egui::Color32::WHITE)
                .strong(),
        )
        .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30)))
    };

    let lobby_btn_resp = ui.add_sized(
        [ui.available_width(), 36.0],
        play_button("Create a Lobby"),
    );

    if lobby_btn_resp.clicked() {
        debug!("[MENU] 'Create a Lobby' clicked, setting menu state to LobbySelection");
        #[cfg(feature = "solana")]
        if let Some(ref mut lobby) = ctx_menu.solana_lobby {
            lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
            lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Idle;
            lobby.wager_sol = 0.0;
        }
        ctx_menu.menu_state.set(crate::core::MenuState::LobbySelection);
    }

    ui.add_space(8.0);

    let join_btn_resp = ui.add_sized(
        [ui.available_width(), 36.0],
        play_button("Join a Lobby"),
    );

    if join_btn_resp.clicked() {
        ctx_menu.competitive_menu.show_join_popup = true;
    }

    ui.add_space(5.0);

    // Play the Computer Section
    ui.set_width(ui.available_width());
    ui.vertical(|ui| {
        if ui.add_sized(
            [ui.available_width(), 36.0],
            play_button("Play against the Computer"),
        ).clicked() {
            ctx_menu.competitive_menu.show_ai_setup = true;
        }

        ui.add_space(5.0);
    });
}

/// Render the TOURNAMENTS bottom card.
pub(super) fn render_tournaments_box(ui: &mut egui::Ui, ctx_menu: &mut MainMenuUIContext) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.set_height(250.0);
        ui.vertical_centered(|ui| {
            ui.heading(
                egui::RichText::new("TOURNAMENTS")
                    .size(16.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
        });
        ui.add_space(6.0);

        if ui.add_sized(
            [ui.available_width(), 24.0],
            egui::Button::new(egui::RichText::new("Browse Tournaments").size(11.0).color(egui::Color32::WHITE))
                .fill(egui::Color32::from_rgb(70, 100, 150))
                .corner_radius(4.0),
        ).clicked() {
            ctx_menu.menu_state.set(crate::core::MenuState::Tournaments);
        }

        ui.add_space(6.0);
        ui.vertical(|ui| {
            #[cfg(feature = "solana")]
            {
                if let Some(ref tc) = ctx_menu.tournament_client {
                    if tc.available_tournaments.is_empty() {
                        ui.label(
                            egui::RichText::new("No active tournaments")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(120, 120, 120))
                                .italics(),
                        );
                    } else {
                        for t in tc.available_tournaments.iter().take(3) {
                            let entry = if t.entry_fee_lamports == 0 {
                                "Free".to_string()
                            } else {
                                format!("{:.3} SOL", t.entry_fee_lamports as f64 / 1e9)
                            };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&t.name).size(11.0).color(egui::Color32::WHITE).strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(egui::RichText::new(entry).size(10.0).color(egui::Color32::from_rgb(150, 200, 150)));
                                });
                            });
                            ui.add_space(4.0);
                        }
                    }
                } else {
                    ui.label(egui::RichText::new("No active tournaments").size(11.0).color(egui::Color32::from_rgb(120, 120, 120)).italics());
                }
            }
            #[cfg(not(feature = "solana"))]
            ui.label(egui::RichText::new("No active tournaments").size(11.0).color(egui::Color32::from_rgb(120, 120, 120)).italics());
        });
    });
}

/// Render the UPDATES bottom card.
pub(super) fn render_updates_box(ui: &mut egui::Ui, box_height: f32) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.set_height(box_height);
        ui.vertical_centered(|ui| {
            ui.heading(
                egui::RichText::new("UPDATES")
                    .size(16.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(
                    egui::RichText::new("XFChess Released stay tuned for updates!")
                        .size(14.0)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200))
                        .italics(),
                );
            });
        });
    });
}

/// Render the QUICK PLAY card with locked online wager tiers.
pub(super) fn render_quick_pairing_section(ui: &mut egui::Ui, _ctx_menu: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("QUICK PLAY")
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );
    });
    ui.add_space(15.0);

    #[cfg(feature = "solana")]
    let rate_snapshot = _ctx_menu
        .sol_gbp_rate
        .as_ref()
        .and_then(|rate| rate.snapshot().cloned());

    #[cfg(feature = "solana")]
    let preset_wagers = [
        ("£2 Wager", 2.0_f64),
        ("£5 Wager", 5.0_f64),
        ("£10 Wager", 10.0_f64),
    ];

    #[cfg(feature = "solana")]
    for (name, gbp) in preset_wagers {
        let sol_value = rate_snapshot
            .as_ref()
            .map(|rate| rate.sol_per_gbp * gbp)
            .unwrap_or(match gbp as u32 {
                2 => 0.05,
                5 => 0.12,
                _ => 0.25,
            });
        let btn_text = format!("{} — {:.3} SOL", name, sol_value);
        let resp = ui.add_sized(
            [ui.available_width(), 36.0],
            egui::Button::new(
                egui::RichText::new(btn_text)
                    .size(13.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .fill(egui::Color32::from_rgb(40, 100, 40))
            .corner_radius(8.0)
            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))),
        ).on_hover_text("Launch a wagered Solana lobby with this preset tier");

        if resp.clicked() {
            info!("[MENU] Quick-play wager {} clicked — opening Solana lobby", name);
            #[cfg(feature = "solana")]
            {
                let wallet_pubkey = _ctx_menu
                    .solana_state
                    .as_ref()
                    .and_then(|state| state.wallet_pubkey);

                if let Some(wallet_pubkey) = wallet_pubkey {
                    if let Some(ref mut lobby) = _ctx_menu.solana_lobby {
                        lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
                        lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Pending;
                        lobby.wager_sol = sol_value as f32;
                        lobby.cached_display_name = Some(_ctx_menu.player_identity.display_name().to_string());
                        lobby.cached_node_id = _ctx_menu.network_state.as_ref().and_then(|ns| {
                            ns.node_id.map(|id| bs58::encode(id.as_bytes()).into_string())
                        });

                        let rpc_url = lobby.cached_rpc_url.clone();
                        let wager_lamports = lobby.wager_lamports();
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        crate::multiplayer::solana::lobby::spawn_create_game(
                            rpc_url,
                            wallet_pubkey,
                            wager_lamports,
                            tx,
                        );
                        lobby.tx_rx = Some(rx);
                        _ctx_menu.menu_state.set(crate::core::MenuState::SolanaLobby);
                    }
                } else {
                    #[cfg(feature = "solana")]
                    crate::multiplayer::solana::tauri_signer::open_wallet_browser();
                    if let Some(ref mut lobby) = _ctx_menu.solana_lobby {
                        lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
                        lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Idle;
                        lobby.wager_sol = sol_value as f32;
                    }
                    _ctx_menu.menu_state.set(crate::core::MenuState::SolanaLobby);
                }
            }
            #[cfg(not(feature = "solana"))]
            bevy::prelude::info!("Solana feature is disabled, cannot open wager lobby.");
        }
        ui.add_space(5.0);
    }

    #[cfg(not(feature = "solana"))]
    {
        let wagers = [("£2 Wager", 0.05), ("£5 Wager", 0.12), ("£10 Wager", 0.25)];
        for (name, _stake) in wagers {
            let btn_text = format!("{} (Locked)", name);
            let resp = ui.add_sized(
                [ui.available_width(), 36.0],
                egui::Button::new(
                    egui::RichText::new(btn_text)
                        .size(13.0)
                        .color(egui::Color32::from_rgb(120, 120, 120))
                        .strong(),
                )
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 150))
                .corner_radius(8.0)
                .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))),
            ).on_hover_text("Connect wallet to wager");

            if resp.clicked() {
                info!("[MENU] Wager {} clicked — wallet not connected, opening wallet popup", name);
                bevy::prelude::info!("Solana feature is disabled, cannot open wager lobby.");
            }
            ui.add_space(5.0);
        }
    }
}

/// Render the NEWS card with the cached screenshot banner.
pub(super) fn render_news_section(ui: &mut egui::Ui, box_width: f32, news_banner: &mut NewsBannerState) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("NEWS")
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );
    });
    ui.add_space(10.0);

    // The screenshot is a wide banner, so we keep the section borderless and
    // let the image define the visual weight of the card.
    ui.vertical_centered(|ui| {
        if let Some(texture_id) = ensure_news_banner_texture(ui.ctx(), news_banner) {
            let banner_w = box_width * 0.98;
            let banner_h = banner_w * 0.50;
            let (rect, response) = ui.allocate_exact_size(egui::vec2(banner_w, banner_h), egui::Sense::click());

            ui.painter().image(
                texture_id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            if response.clicked() {
                info!("[MENU] News banner clicked - opening release page");
                if let Err(e) = webbrowser::open("http://localhost:5173/news/release") {
                    warn!("[MENU] Failed to open release notes: {}", e);
                }
            }
        } else {
            let fallback_size = egui::vec2(box_width * 0.98, box_width * 0.50);
            let (rect, response) = ui.allocate_exact_size(fallback_size, egui::Sense::click());
            ui.painter().rect_filled(rect, 8.0, egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "XFChess released",
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );
            if response.clicked() {
                info!("[MENU] News banner fallback clicked - opening release page");
                if let Err(e) = webbrowser::open("http://localhost:5173/news/release") {
                    warn!("[MENU] Failed to open release notes: {}", e);
                }
            }
        }
    });
}

/// Render the LEARN card — a borderless square that hosts the `XfAnimatePlugin`
/// 3D showcase camera. The allocated rect (in physical pixels) is written to
/// `LearnViewportRect` so the camera viewport tracks egui's layout.
pub(super) fn render_learn_section(
    ui: &mut egui::Ui,
    box_width: f32,
    learn_viewport: &mut crate::xf_animate::LearnViewportRect,
) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("LEARN")
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );
    });
    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Immortal Zugzwang Game")
            .size(14.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new("Sämisch vs Nimzowitsch, 1923")
            .size(11.0)
            .color(egui::Color32::from_rgb(150, 150, 150))
            .italics(),
    );
    ui.add_space(6.0);

    // Reserve a perfect square for the mini-board viewport. Guard against
    // pathological layouts (negative/zero available width) — egui panics on
    // negative dimensions, and wgpu rejects scissor rects beyond the render
    // target, so we simply hide the camera in those cases.
    let available = ui.available_width().max(0.0);
    let side = box_width.min(available).max(0.0) * 0.92;
    const MIN_SIDE: f32 = 48.0;

    if side < MIN_SIDE {
        learn_viewport.rect_px = None;
    } else {
        ui.vertical_centered(|ui| {
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(side, side),
                egui::Sense::hover(),
            );
            let ppp = ui.ctx().pixels_per_point();
            learn_viewport.rect_px =
                Some(crate::xf_animate::viewport::egui_rect_to_pixels(rect, ppp));
        });
    }
}

/// Render the legacy middle lobby section with live VPS listings and type filter.
#[allow(dead_code)]
pub(super) fn render_lobby_section(ui: &mut egui::Ui, ctx_menu: &mut MainMenuUIContext) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.set_height(250.0);
        ui.vertical_centered(|ui| {
            ui.heading(
                egui::RichText::new("LOBBY")
                    .size(16.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            let mut tournaments_found = false;
            if let Some(vps_state) = ctx_menu.p2p_vps_state.as_ref() {
                for listing in &vps_state.cached_games {
                    if listing.game_type == "tournament" {
                        tournaments_found = true;
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&listing.display_name)
                                        .size(12.0)
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
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(150, 200, 150)),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add_sized(
                                    [60.0, 24.0],
                                    egui::Button::new(
                                        egui::RichText::new("Join")
                                            .size(11.0)
                                            .color(egui::Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(egui::Color32::from_rgb(100, 200, 100))
                                    .corner_radius(6.0)
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))),
                                ).clicked() {
                                    ctx_menu.next_state.set(GameState::InGame);
                                }
                            });
                        });
                        ui.add_space(8.0);
                    }
                }
            }

            if !tournaments_found {
                ui.label(
                    egui::RichText::new("No active tournaments")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(120, 120, 120))
                        .italics(),
                );
            }
        });
    });
}
