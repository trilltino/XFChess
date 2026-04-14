//! Solana-specific UI components for competitive mode
//!
//! Includes wallet connection status, ELO stats, and wagering UI.

use crate::multiplayer::solana::addon::{
    CompetitiveMatchState, SolanaGameSync, SolanaProfile, SolanaWallet,
};
use crate::ui::popup::{GamePopup, GamePopupQueue};
use crate::ui::styles::UiColors;
use bevy::prelude::*;
use bevy_egui::egui;

/// Renders the Solana sidebar with wallet info and game stats
pub fn render_solana_panel(
    ui: &mut egui::Ui,
    wallet: &mut SolanaWallet,
    sync: &mut SolanaGameSync,
    competitive: &mut CompetitiveMatchState,
    profile: &SolanaProfile,
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
                        .small_button("📋")
                        .on_hover_text(format!("Copy address\n{}", pk_str))
                        .clicked()
                    {
                        ui.output_mut(|o| {
                            o.commands.push(egui::OutputCommand::CopyText(pk_str.clone()))
                        });
                    }
                });
                if wallet.keypair.is_some() {
                    ui.label(
                        egui::RichText::new("⚠ Hot wallet — fund on devnet faucet")
                            .size(10.0)
                            .color(UiColors::WARNING),
                    );
                }
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
                ui.label(egui::RichText::new("Connect your Solana wallet to play:").size(11.0).color(UiColors::TEXT_SECONDARY));
                ui.add_space(5.0);
                
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("👻 Phantom").strong()).on_hover_text("Connect via Phantom Extension").clicked() {
                        // The automated system in systems.rs already polls for pubkey via Tauri bridge
                        info!("[WALLET] User selected Phantom - awaiting connection...");
                    }
                    ui.add_space(5.0);
                    if ui.button(egui::RichText::new("☀️ Solflare").strong()).on_hover_text("Connect via Solflare Extension").clicked() {
                        info!("[WALLET] User selected Solflare - awaiting connection...");
                    }
                });
            }
        });

        ui.add_space(10.0);

        // --- Stats Section ---
        ui.group(|ui| {
            ui.label(egui::RichText::new("ON-CHAIN STATS").strong());
            ui.label(format!("ELO: {}", profile.elo));
            ui.label(format!("Games: {}", profile.games_played()));
            ui.label(format!(
                "W {}  L {}  D {}",
                profile.wins, profile.losses, profile.draws
            ));
            ui.add_space(5.0);
            if ui.button("Manage Profile (Web)").clicked() {
                let _ = webbrowser::open("http://localhost:5173/profile");
            }
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

/// One-shot system: fires when entering InGame with a hot (browser) wallet.
/// Reads XFCHESS_HOT_WALLET env var (set by Tauri to the wallet pubkey).
pub fn show_hot_wallet_faucet_popup(
    mut queue: ResMut<GamePopupQueue>,
) {
    let Ok(pk_str) = std::env::var("XFCHESS_HOT_WALLET") else { return };
    if pk_str.is_empty() { return; }

    // Prevent duplicate faucet popups in the queue
    if queue.entries.iter().any(|p| p.title == "Fund Your Hot Wallet") {
        return;
    }

    queue.push(
        GamePopup::warning(
            "Fund Your Hot Wallet",
            "This is a fresh hot wallet generated for this session.\n\
             Send devnet SOL from the faucet to play ranked games.",
        )
        .with_copy(pk_str.clone())
        .with_url(
            "https://faucet.solana.com/".to_string(),
            "Open Faucet →",
        )
        .persistent(),
    );
}
