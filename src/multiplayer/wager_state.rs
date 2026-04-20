//! Wager State Integration
//!
//! This module handles wager information from the web UI via CLI arguments
//! and displays it in the game UI.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::GameConfig;

/// Resource that holds wager information passed from Web UI
#[derive(Resource, Debug, Clone)]
pub struct WagerState {
    /// Game ID from on-chain
    pub game_id: Option<u64>,
    /// Wager amount in SOL
    pub wager_amount: Option<f64>,
    /// Total pot (wager * 2)
    pub total_pot: Option<f64>,
    /// Player's color
    pub player_color: Option<String>,
    /// Game PDA address
    pub game_pda: Option<String>,
    /// Whether wager info is loaded
    pub is_loaded: bool,
}

impl Default for WagerState {
    fn default() -> Self {
        Self {
            game_id: None,
            wager_amount: None,
            total_pot: None,
            player_color: None,
            game_pda: None,
            is_loaded: false,
        }
    }
}

impl WagerState {
    /// Initialize from GameConfig (CLI arguments)
    pub fn from_config(config: &GameConfig) -> Self {
        let total_pot = config.wager_amount.map(|w| w * 2.0);

        Self {
            game_id: config.game_id,
            wager_amount: config.wager_amount,
            total_pot,
            player_color: config.player_color.map(|c| format!("{:?}", c)),
            game_pda: config.game_pda.clone(),
            is_loaded: config.wager_amount.is_some(),
        }
    }

    /// Get formatted wager display string
    pub fn wager_display(&self) -> String {
        match self.wager_amount {
            Some(amount) => format!("{:.3} SOL", amount),
            None => "Free Game".to_string(),
        }
    }

    /// Get formatted pot display string
    pub fn pot_display(&self) -> String {
        match self.total_pot {
            Some(amount) => format!("{:.3} SOL", amount),
            None => "0 SOL".to_string(),
        }
    }

    /// Check if this is a wager game
    pub fn has_wager(&self) -> bool {
        self.wager_amount.map_or(false, |w| w > 0.0)
    }
}

/// Plugin for wager integration
pub struct WagerPlugin;

impl Plugin for WagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WagerState>()
            .add_systems(Startup, initialize_wager_state)
            .add_systems(Update, wager_ui_system);
    }
}

/// Initialize wager state from CLI config
fn initialize_wager_state(config: Res<GameConfig>, mut wager_state: ResMut<WagerState>) {
    *wager_state = WagerState::from_config(&config);

    if wager_state.is_loaded {
        info!(
            "[WagerState] Loaded wager: {} | Pot: {}",
            wager_state.wager_display(),
            wager_state.pot_display()
        );
    }
}

/// UI system that displays wager info in-game
fn wager_ui_system(wager_state: Res<WagerState>, mut contexts: EguiContexts) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::PlayerColor;

    #[test]
    fn test_wager_state_from_config() {
        let config = GameConfig {
            game_id: Some(12345),
            player_color: Some(PlayerColor::White),
            wager_amount: Some(0.1),
            ..Default::default()
        };

        let state = WagerState::from_config(&config);

        assert_eq!(state.game_id, Some(12345));
        assert_eq!(state.wager_amount, Some(0.1));
        assert_eq!(state.total_pot, Some(0.2));
        assert!(state.is_loaded);
    }

    #[test]
    fn test_wager_display() {
        let state = WagerState {
            wager_amount: Some(0.5),
            ..Default::default()
        };

        assert_eq!(state.wager_display(), "0.500 SOL");
    }

    #[test]
    fn test_has_wager() {
        let state_with = WagerState {
            wager_amount: Some(0.1),
            ..Default::default()
        };
        assert!(state_with.has_wager());

        let state_without = WagerState {
            wager_amount: None,
            ..Default::default()
        };
        assert!(!state_without.has_wager());
    }
}
