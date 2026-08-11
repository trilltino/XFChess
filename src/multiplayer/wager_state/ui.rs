use super::state::WagerState;
use crate::multiplayer::solana::wager_rate::SolUsdRate;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

/// UI system that displays wager info in-game
pub fn wager_ui_system(
    wager_state: Res<WagerState>,
    sol_usd_rate: Res<SolUsdRate>,
    mut contexts: EguiContexts,
) {
    // Only show if wager info is loaded
    if !wager_state.is_loaded {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // USD is the primary display currency everywhere else in the client (see
    // wager_rate.rs) — this HUD only fell back to raw SOL because it predates
    // that convention. Falls back to a plain SOL amount when no live rate has
    // loaded yet, same as the wager lobby.
    let usd_or_sol = |sol: f64| -> String {
        match sol_usd_rate.usd_for_sol(sol) {
            Some(usd) => format!("${:.2}", usd),
            None => format!("{:.4} SOL", sol),
        }
    };

    // Create a top-right panel for wager info
    egui::Window::new("Wager Info")
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
        .default_width(200.0)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Game ID
                if let Some(id) = wager_state.game_id {
                    ui.label(format!("Game ID: {}", id));
                }

                ui.separator();

                // Wager amount
                ui.horizontal(|ui| {
                    ui.label("Your Wager:");
                    let text = wager_state
                        .wager_amount
                        .map(usd_or_sol)
                        .unwrap_or_else(|| wager_state.wager_display());
                    ui.label(
                        egui::RichText::new(text)
                            .color(egui::Color32::GOLD)
                            .strong(),
                    );
                });

                // Total pot
                ui.horizontal(|ui| {
                    ui.label("Total Pot:");
                    let text = wager_state
                        .total_pot
                        .map(usd_or_sol)
                        .unwrap_or_else(|| wager_state.pot_display());
                    ui.label(
                        egui::RichText::new(text)
                            .color(egui::Color32::GREEN)
                            .strong(),
                    );
                });

                ui.separator();

                // Fee breakdown (only show for ranked/wager/tournament games)
                if wager_state.match_type.as_deref() != Some("Free") {
                    ui.label(egui::RichText::new("Fee Breakdown").strong().size(13.0));
                    ui.add_space(4.0);

                    // Country fee
                    if let Some(country_fee) = wager_state.country_fee {
                        ui.horizontal(|ui| {
                            ui.label("Country Fee:");
                            ui.label(
                                egui::RichText::new(usd_or_sol(country_fee))
                                    .color(egui::Color32::LIGHT_BLUE)
                                    .small(),
                            );
                        });
                    }

                    // ELO fee
                    if let Some(elo_fee) = wager_state.elo_fee {
                        ui.horizontal(|ui| {
                            ui.label("ELO Fee:");
                            ui.label(
                                egui::RichText::new(usd_or_sol(elo_fee))
                                    .color(egui::Color32::LIGHT_BLUE)
                                    .small(),
                            );
                        });
                    }

                    ui.separator();

                    // Total fees
                    let total_fees =
                        wager_state.country_fee.unwrap_or(0.0) + wager_state.elo_fee.unwrap_or(0.0);
                    if total_fees > 0.0 {
                        ui.horizontal(|ui| {
                            ui.label("Total Fees:");
                            ui.label(
                                egui::RichText::new(usd_or_sol(total_fees))
                                    .color(egui::Color32::YELLOW)
                                    .strong()
                                    .small(),
                            );
                        });
                    }
                }

                // Player color
                if let Some(ref player_color) = wager_state.player_color {
                    ui.horizontal(|ui| {
                        ui.label("Playing as:");
                        let (color_text, color_value) = if player_color == "White" {
                            ("White", egui::Color32::WHITE)
                        } else {
                            ("Black", egui::Color32::BLACK)
                        };
                        ui.label(egui::RichText::new(color_text).color(color_value).strong());
                    });
                }

                // Warning for wager games
                if wager_state.has_wager() {
                    ui.separator();
                    ui.label(
                        egui::RichText::new("This is a real money game!")
                            .color(egui::Color32::YELLOW)
                            .small(),
                    );
                }
            });
        });
}
