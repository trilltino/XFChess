//! Solana-specific UI components for competitive mode
//!
//! Includes wallet connection status, ELO stats, and wagering UI.

use crate::game::resources::Players;
use crate::multiplayer::solana_addon::{
    CompetitiveMatchState, SolanaGameSync, SolanaProfile, SolanaWallet,
};
use crate::ui::styles::UiColors;
use bevy::prelude::*;
use bevy_egui::egui;
use solana_sdk::signature::Signer;

/// Renders the Solana sidebar with wallet info and game stats
pub fn render_solana_panel(
    ui: &mut egui::Ui,
    wallet: &mut SolanaWallet,
    sync: &mut SolanaGameSync,
    competitive: &mut CompetitiveMatchState,
    players: &Players,
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
                ui.label(egui::RichText::new(short_pk).color(UiColors::TEXT_SECONDARY));
                if ui.button("Disconnect").clicked() {
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
                if ui.button("Connect Local Wallet").clicked() {
                    use solana_sdk::signature::Keypair;
                    use std::sync::Arc;
                    let keypair = Arc::new(Keypair::new());
                    wallet.pubkey = Some(keypair.pubkey());
                    wallet.keypair = Some(keypair);
                }
            }
        });

        ui.add_space(10.0);

        // --- Stats Section ---
        ui.group(|ui| {
            ui.label(egui::RichText::new("ON-CHAIN STATS").strong());
            ui.label(format!("ELO: {}", profile.elo));
            ui.label(format!("Games: {}", profile.games_played));
            ui.label(format!(
                "W {}  L {}  D {}",
                profile.wins, profile.losses, profile.draws
            ));
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
