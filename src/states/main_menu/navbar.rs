//! Top navigation bar for the website-style main menu.
//!
//! Renders the brand logo, the primary nav links (Spectator, Tournaments,
//! Community, Source Code, Controls) and the right-side identity controls
//! (Connect Wallet / Logout + username display). Popups and screen
//! transitions are triggered by mutating the shared `MainMenuUIContext`.

use super::*;
use bevy_egui::egui;
use tracing::{info, warn};

/// Render the website-style navbar at the top of the main menu.
pub(super) fn render_navbar(ctx: &egui::Context, ctx_menu: &mut MainMenuUIContext) {
    egui::TopBottomPanel::top("navbar")
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(45, 45, 45, 210), // Seamless grey (matches CentralPanel)
            inner_margin: egui::Margin::symmetric(20, 8),
            outer_margin: egui::Margin::ZERO,
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // === BRAND LOGO ===
                if let Some(texture_id) = ensure_brand_logo_texture(ui.ctx(), &mut ctx_menu.brand_logo) {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
                    ui.painter().image(
                        texture_id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    ui.label(
                        egui::RichText::new("XFChess")
                            .size(16.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                }

                ui.add_space(18.0);

                // === LEFT SIDE: SPECTATOR | REPLAY | COMMUNITY | SOURCE CODE ===
                ui.horizontal(|ui| {
                    if nav_link(ui, "Spectator") {
                        info!("[MENU] Spectator clicked - opening spectator popup");
                        ctx_menu.competitive_menu.show_spectator_popup = true;
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Replay") {
                        info!("[MENU] Replay clicked - opening file picker");
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Chess PGN", &["pgn"])
                            .set_title("Select a PGN file to replay")
                            .pick_file()
                        {
                            match std::fs::read_to_string(&path) {
                                Ok(text) => {
                                    match nimzovich_engine::parse_pgn(&text) {
                                        Ok(pgn_game) => {
                                            info!("[REPLAY] Loaded PGN with {} moves", pgn_game.moves.len());
                                            ctx_menu.commands.insert_resource(
                                                crate::game::replay::ParsedPgnGameResource { inner: pgn_game }
                                            );
                                            *ctx_menu.core_mode = crate::core::GameMode::PgnReplay;
                                            ctx_menu.next_state.set(crate::core::GameState::InGame);
                                        }
                                        Err(e) => {
                                            warn!("[REPLAY] Failed to parse PGN: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("[REPLAY] Failed to read file: {}", e);
                                }
                            }
                        }
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Tournaments") {
                        info!("[MENU] Tournaments clicked");
                        ctx_menu.menu_state.set(crate::core::MenuState::Tournaments);
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Community") {
                        info!("[MENU] Community clicked - opening Telegram");
                        if let Err(e) = webbrowser::open("https://t.me/+IBdo42qMPqM4Y2Vk") {
                            warn!("[MENU] Failed to open Telegram: {}", e);
                        }
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Source Code") {
                        info!("[MENU] Source Code clicked - opening GitHub");
                        if let Err(e) = webbrowser::open("https://github.com/trilltino/XFChess") {
                            warn!("[MENU] Failed to open GitHub repository: {}", e);
                        }
                    }
                    ui.add_space(30.0);
                    if nav_link(ui, "Controls") {
                        info!("[MENU] Controls clicked - opening controls popup");
                        ctx_menu.competitive_menu.show_controls_popup = true;
                    }
                });

                // === RIGHT SIDE: USERNAME DISPLAY AND AUTH BUTTONS ===
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ctx_menu.player_identity.username.is_some() {
                        if ui.add_sized(
                            [90.0, 30.0],
                            egui::Button::new(
                                egui::RichText::new("Logout")
                                    .size(14.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(120, 70, 70))
                            .corner_radius(6.0)
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))),
                        ).clicked() {
                            info!("[MENU] Logout clicked");
                            ctx_menu.player_identity.username = None;
                            ctx_menu.auth_state.token = None;
                        }
                    } else {
                        if ui.add_sized(
                            [160.0, 30.0],
                            egui::Button::new(
                                egui::RichText::new("Connect Wallet")
                                    .size(14.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(70, 130, 180))
                            .corner_radius(6.0)
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))),
                        ).clicked() {
                            info!("[MENU] Connect Wallet clicked — opening Tauri wallet popup");
                            #[cfg(feature = "solana")]
                            crate::multiplayer::solana::tauri_signer::open_wallet_browser();
                            #[cfg(not(feature = "solana"))]
                            bevy::prelude::info!("Solana feature is disabled, cannot open wallet.");
                        }
                    }

                    ui.add_space(10.0);

                    // Username Display
                    if let Some(ref username) = ctx_menu.player_identity.username {
                        if !username.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("@{}", username))
                                    .size(16.0)
                                    .color(egui::Color32::from_rgb(100, 200, 255))
                                    .strong()
                            );
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new("Player:")
                                    .size(14.0)
                                    .color(egui::Color32::GRAY)
                            );
                        } else {
                            ui.label(
                                egui::RichText::new("Guest")
                                    .size(14.0)
                                    .color(egui::Color32::GRAY)
                            );
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("Guest")
                                .size(14.0)
                                .color(egui::Color32::GRAY)
                        );
                    }
                });
            });
        });
}

/// Navbar link helper — plain clickable text, no box.
pub(super) fn nav_link(ui: &mut egui::Ui, text: &str) -> bool {
    let response = ui.add(
        egui::Label::new(
            egui::RichText::new(text)
                .size(14.0)
                .color(egui::Color32::from_rgb(200, 200, 200)),
        )
        .sense(egui::Sense::click()),
    );

    if response.hovered() {
        ui.painter().text(
            response.rect.left_bottom(),
            egui::Align2::LEFT_BOTTOM,
            text,
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }

    response.clicked()
}

/// Navbar button helper (currently unused but kept for parity with the old menu).
#[allow(dead_code)]
pub(super) fn nav_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let response = ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .size(14.0)
                .color(egui::Color32::WHITE),
        )
        .fill(egui::Color32::from_rgba_unmultiplied(60, 60, 60, 200))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 100))),
    );

    if response.hovered() {
        ui.painter().rect(
            response.rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
            egui::Stroke::NONE,
            egui::epaint::StrokeKind::Middle,
        );
    }

    response
}
