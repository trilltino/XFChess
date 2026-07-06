//! Solana-specific UI components for competitive mode
//!
//! Includes wallet connection status, ELO stats, and wagering UI.

use crate::multiplayer::solana::addon::{
    CompetitiveMatchState, SolanaGameSync, SolanaProfile, SolanaWallet,
};
use crate::multiplayer::solana::integration::state::{ProfileStatus, SolanaIntegrationState};
use crate::multiplayer::solana::integration::systems::GlobalSessionActive;
use crate::ui::styles::UiColors;
use bevy::prelude::*;
use bevy_egui::egui;

/// Profile status chip colours
fn profile_status_color(status: ProfileStatus) -> egui::Color32 {
    match status {
        ProfileStatus::HasProfileWithUsername => egui::Color32::from_rgb(34, 197, 94),
        ProfileStatus::HasProfileNoUsername => egui::Color32::from_rgb(251, 191, 36),
        ProfileStatus::NoProfile => egui::Color32::from_rgb(239, 68, 68),
        ProfileStatus::Unknown => egui::Color32::GRAY,
    }
}

fn profile_status_label(status: ProfileStatus) -> &'static str {
    match status {
        ProfileStatus::HasProfileWithUsername => "Profile OK",
        ProfileStatus::HasProfileNoUsername => "No Username",
        ProfileStatus::NoProfile => "No Profile",
        ProfileStatus::Unknown => "Checking…",
    }
}

/// Renders the Solana sidebar with wallet info and game stats
pub fn render_solana_panel(
    ui: &mut egui::Ui,
    wallet: &mut SolanaWallet,
    sync: &mut SolanaGameSync,
    competitive: &mut CompetitiveMatchState,
    profile: &SolanaProfile,
    solana_state: &SolanaIntegrationState,
    profile_view_open: &mut bool,
    global_session: Option<&GlobalSessionActive>,
    global_session_pending: bool,
) {
    ui.vertical(|ui| {
        ui.heading(egui::RichText::new("SOLANA COMPETITIVE").color(UiColors::ACCENT_GOLD));
        ui.add_space(10.0);

        // --- Wallet Section ---
        ui.group(|ui| {
            ui.label(egui::RichText::new("WALLET").strong());
            if let Some(pubkey) = &wallet.pubkey {
                let pk_str = pubkey.to_string();
                let short_pk = format!("{}...{}", &pk_str[..6], &pk_str[pk_str.len() - 4..]);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&short_pk).color(UiColors::TEXT_SECONDARY));
                    if ui
                        .small_button("")
                        .on_hover_text(format!("Copy address\n{}", pk_str))
                        .clicked()
                    {
                        ui.output_mut(|o| {
                            o.commands
                                .push(egui::OutputCommand::CopyText(pk_str.clone()))
                        });
                    }
                });
                if ui.small_button("Disconnect").clicked() {
                    wallet.pubkey = None;
                    wallet.keypair = None;
                    wallet.ranked_active = false;
                    wallet.tournament_match_id = None;
                    sync.game_id = None;
                    sync.moves_submitted = 0;
                    sync.wager_amount = 0;
                }
            } else {
                ui.colored_label(UiColors::DANGER, "Not Connected");
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new("Connect your Solana wallet to play:")
                        .size(11.0)
                        .color(UiColors::TEXT_SECONDARY),
                );
                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(egui::RichText::new(" Phantom").strong())
                        .on_hover_text("Connect via Phantom Extension")
                        .clicked()
                    {
                        crate::multiplayer::solana::tauri_signer::open_wallet_browser();
                        info!("[WALLET] User selected Phantom - opening Tauri popup...");
                    }
                    ui.add_space(5.0);
                    if ui
                        .button(egui::RichText::new("? Solflare").strong())
                        .on_hover_text("Connect via Solflare Extension")
                        .clicked()
                    {
                        crate::multiplayer::solana::tauri_signer::open_wallet_browser();
                        info!("[WALLET] User selected Solflare - opening Tauri popup...");
                    }
                });
            }
        });

        ui.add_space(10.0);

        // --- Verification Section ---
        if wallet.pubkey.is_some() {
            ui.group(|ui| {
                ui.label(egui::RichText::new("VERIFICATION").strong());

                let status = &wallet.user_status;
                let has_profile = status.as_ref().map(|s| s.has_profile).unwrap_or(false);
                let has_email = status.as_ref().map(|s| s.has_email).unwrap_or(false);
                let has_kyc = status.as_ref().map(|s| s.has_kyc).unwrap_or(false);

                ui.horizontal(|ui| {
                    ui.label(if has_profile { "" } else { "" });
                    ui.label("Profile");
                });
                ui.horizontal(|ui| {
                    ui.label(if has_email { "" } else { "" });
                    ui.label("Email");
                });
                ui.horizontal(|ui| {
                    ui.label(if has_kyc { "" } else { "" });
                    ui.colored_label(
                        if has_kyc {
                            UiColors::SUCCESS
                        } else {
                            UiColors::DANGER
                        },
                        "KYC — required for wagered play",
                    );
                });

                ui.add_space(5.0);
                let backend_url = std::env::var("BACKEND_URL")
                    .unwrap_or_else(|_| "http://178.104.55.19".to_string());
                let profile_url = format!("{}/profile", backend_url);
                if ui.button("Complete at xfchess.gg/profile ?").clicked() {
                    let _ = webbrowser::open(&profile_url);
                }
            });
        }

        // --- Global Session Status ---
        if wallet.pubkey.is_some() {
            ui.add_space(6.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new("GLOBAL SESSION").strong());
                if global_session_pending {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Verifying…")
                                .size(11.0)
                                .color(UiColors::TEXT_SECONDARY),
                        );
                    });
                } else if let Some(gs) = global_session {
                    let short = if gs.session_pubkey.len() > 12 {
                        format!(
                            "{}…{}",
                            &gs.session_pubkey[..6],
                            &gs.session_pubkey[gs.session_pubkey.len() - 4..]
                        )
                    } else {
                        gs.session_pubkey.clone()
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                        ui.label(
                            egui::RichText::new(short)
                                .size(10.0)
                                .color(UiColors::TEXT_SECONDARY)
                                .monospace(),
                        );
                    });
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 200, 50),
                        "No session — authorize to skip wallet popups",
                    );
                }
            });
        }

        ui.add_space(10.0);

        // --- Stats Section ---
        ui.group(|ui| {
            ui.label(egui::RichText::new("ON-CHAIN STATS").strong());

            // Profile status chip
            let status = solana_state.profile_status;
            let chip_color = profile_status_color(status);
            let chip_label = profile_status_label(status);
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.colored_label(chip_color, chip_label);
            });

            // Display name if available
            if let Some(ref name) = solana_state.cached_display_name {
                ui.label(format!("Username: {}", name));
            }

            let display_elo = if solana_state.cached_elo > 0 {
                solana_state.cached_elo as u32
            } else {
                profile.elo
            };
            ui.label(format!("ELO: {}", display_elo));
            ui.label(format!("Games: {}", profile.games_played()));
            ui.label(format!(
                "W {}  L {}  D {}",
                profile.wins, profile.losses, profile.draws
            ));
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                if ui.button("View Profile").clicked() {
                    *profile_view_open = true;
                }
                let backend_url = std::env::var("BACKEND_URL")
                    .unwrap_or_else(|_| "http://178.104.55.19".to_string());
                let profile_url = format!("{}/profile", backend_url);
                if ui.small_button("Web ↗").clicked() {
                    let _ = webbrowser::open(&profile_url);
                }
            });
        });

        ui.add_space(10.0);

        // --- Wager Section ---
        if competitive.active || sync.game_id.is_some() {
            ui.group(|ui| {
                ui.label(egui::RichText::new("ACTIVE WAGER").strong());
                let lamports = if competitive.wager_lamports > 0 {
                    competitive.wager_lamports
                } else {
                    sync.wager_amount
                };
                if lamports > 0 {
                    ui.label(format!("Amount: {} SOL", lamports as f64 / 1_000_000_000.0));
                } else {
                    ui.label("Amount: —");
                }
                if let Some(id) = competitive.game_id.or(sync.game_id) {
                    ui.label(format!("Game ID: {}", id));
                } else {
                    ui.label("Game ID: —");
                }
            });
        } else {
            ui.group(|ui| {
                ui.label(egui::RichText::new("GAME SETUP").strong());
                ui.label("Use Mode Select menu to create or join a match.");
            });
        }

        if competitive.finalizing_on_chain {
            ui.label(
                egui::RichText::new("Finalizing match on-chain...").color(UiColors::TEXT_SECONDARY),
            );
        } else if let Some(id) = competitive.last_finalized_game_id {
            ui.label(
                egui::RichText::new(format!("Last finalized match: #{}", id))
                    .color(UiColors::ACCENT_GOLD),
            );
        }

        if let Some(err) = &competitive.last_error {
            ui.colored_label(UiColors::DANGER, err);
        }
    });
}
