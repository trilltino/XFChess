use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use super::state::WagerState;

/// UI system that displays wager info in-game
pub fn wager_ui_system(wager_state: Res<WagerState>, mut contexts: EguiContexts) {
    // Only show if wager info is loaded
    if !wager_state.is_loaded {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // Create a top-right panel for wager info
    egui::Window::new("💰 Wager Info")
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
                    ui.label(
                        egui::RichText::new(wager_state.wager_display())
                            .color(egui::Color32::GOLD)
                            .strong(),
                    );
                });

                // Total pot
                ui.horizontal(|ui| {
                    ui.label("Total Pot:");
                    ui.label(
                        egui::RichText::new(wager_state.pot_display())
                            .color(egui::Color32::GREEN)
                            .strong(),
                    );
                });

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
                        egui::RichText::new("⚠️ This is a real money game!")
                            .color(egui::Color32::YELLOW)
                            .small(),
                    );
                }
            });
        });
}
