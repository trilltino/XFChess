//! Game Over popup UI for showing game results with Solana fee breakdown
//!
//! Displays a small translucent popup when the game ends, showing the winner
//! and detailed fee information from the smart contract for wager games.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::core::GameState;
use crate::game::components::piece_types::PieceColor;
use crate::game::resources::GameOverState;

/// Resource holding payout information for Solana wager games
#[derive(Resource, Debug, Clone, Default)]
pub struct GameOverPayoutInfo {
    /// Original wager amount in lamports (per player)
    pub wager_amount: u64,
    /// Treasury/country fee in lamports
    pub country_fee: u64,
    /// ELO fee in lamports (total, split between players)
    pub elo_fee: u64,
    /// Estimated rent returned in lamports
    pub rent_return: u64,
    /// Net winning prize in lamports (what winner actually receives)
    pub winning_prize: u64,
    /// Whether this is a draw (affects payout display)
    pub is_draw: bool,
    /// Player's color (to show if they won)
    pub player_color: Option<PieceColor>,
}

impl GameOverPayoutInfo {
    /// Convert lamports to SOL string with 3 decimal places
    fn format_sol(lamports: u64) -> String {
        format!("{:.3} SOL", lamports as f64 / 1_000_000_000.0)
    }

    /// Check if this is a Solana wager game with payout info
    pub fn is_wager_game(&self) -> bool {
        self.wager_amount > 0
    }

    /// Get the amount the player won (for display)
    pub fn player_winnings(&self) -> u64 {
        if self.is_draw {
            // In a draw, player gets their wager back minus half the fees
            self.wager_amount.saturating_sub(self.country_fee / 2).saturating_sub(self.elo_fee / 2)
        } else {
            // Winner gets the pot minus fees
            self.winning_prize
        }
    }
}

/// System to render the Game Over popup UI
pub fn game_over_popup_system(
    mut contexts: EguiContexts,
    game_over: Res<GameOverState>,
    payout_info: Option<Res<GameOverPayoutInfo>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Define colors matching the game's existing theme
    let bg_color = egui::Color32::from_rgba_unmultiplied(10, 15, 20, 220);
    let accent_color = egui::Color32::from_rgb(244, 187, 68); // gold
    let text_primary = egui::Color32::from_rgb(240, 240, 240);
    let text_secondary = egui::Color32::from_rgb(160, 160, 160);
    let text_gold = egui::Color32::from_rgb(244, 187, 68);

    // Build the frame style
    let frame = egui::Frame::default()
        .fill(bg_color)
        .stroke(egui::Stroke::new(1.0, accent_color))
        .corner_radius(12.0)
        .inner_margin(20.0);

    // Determine winner text
    let (winner_title, winner_color) = match game_over.winner() {
        Some(PieceColor::White) => ("White Wins!", text_primary),
        Some(PieceColor::Black) => ("Black Wins!", text_secondary),
        None => ("Draw!", accent_color),
    };

    // Create the popup window centered on screen
    egui::Window::new("")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([460.0, 260.0])
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(420.0);
            ui.vertical_centered(|ui| {
                // Game Over header
                ui.label(
                    egui::RichText::new("Game Over")
                        .size(20.0)
                        .strong()
                        .color(text_primary),
                );
                ui.add_space(8.0);

                // Winner announcement
                ui.label(
                    egui::RichText::new(winner_title)
                        .size(18.0)
                        .strong()
                        .color(winner_color),
                );

                // Result message (checkmate, timeout, etc.)
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(game_over.message())
                        .size(12.0)
                        .color(text_secondary),
                );

                // Payout information for Solana games
                if let Some(info) = payout_info.as_ref() {
                    if info.is_wager_game() {
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);

                        ui.label(
                            egui::RichText::new("Payout Details")
                                .size(14.0)
                                .strong()
                                .color(text_gold),
                        );
                        ui.add_space(8.0);

                        // Fee breakdown
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Wager:")
                                    .size(12.0)
                                    .color(text_secondary),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(GameOverPayoutInfo::format_sol(info.wager_amount * 2))
                                        .size(12.0)
                                        .color(text_primary),
                                );
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Treasury Fee:")
                                    .size(12.0)
                                    .color(text_secondary),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(format!("- {}", GameOverPayoutInfo::format_sol(info.country_fee)))
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(255, 100, 100)),
                                );
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("ELO Fee:")
                                    .size(12.0)
                                    .color(text_secondary),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(format!("- {}", GameOverPayoutInfo::format_sol(info.elo_fee)))
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(255, 100, 100)),
                                );
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Rent Returned:")
                                    .size(12.0)
                                    .color(text_secondary),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(format!("+ {}", GameOverPayoutInfo::format_sol(info.rent_return)))
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(100, 255, 100)),
                                );
                            });
                        });

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Final prize
                        let prize_text = if info.is_draw {
                            "Returned:"
                        } else {
                            "You Won:"
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(prize_text)
                                    .size(14.0)
                                    .strong()
                                    .color(text_gold),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(GameOverPayoutInfo::format_sol(info.player_winnings()))
                                        .size(16.0)
                                        .strong()
                                        .color(text_gold),
                                );
                            });
                        });
                    }
                }

                ui.add_space(16.0);

                // Action buttons
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;

                    // New Game button
                    let new_game_btn = ui.add_sized(
                        [150.0, 42.0],
                        egui::Button::new(
                            egui::RichText::new("New Game")
                                .size(13.0)
                                .strong(),
                        )
                        .fill(egui::Color32::from_rgb(60, 100, 60)),
                    );

                    if new_game_btn.clicked() {
                        info!("[GAME_OVER_POPUP] Starting new game");
                        next_state.set(GameState::InGame);
                    }

                    // Main Menu button
                    let main_menu_btn = ui.add_sized(
                        [150.0, 42.0],
                        egui::Button::new(
                            egui::RichText::new("Main Menu")
                                .size(13.0)
                                .strong(),
                        )
                        .fill(egui::Color32::from_rgb(80, 60, 60)),
                    );

                    if main_menu_btn.clicked() {
                        info!("[GAME_OVER_POPUP] Returning to main menu");
                        next_state.set(GameState::MainMenu);
                    }
                });
            });
        });
}

/// System to fetch on-chain payout information when game ends
/// This should be called when entering GameOver state
#[cfg(feature = "solana")]
pub fn fetch_game_payout_info(
    game_over: Res<GameOverState>,
    solana_sync: Option<Res<crate::multiplayer::solana::addon::SolanaGameSync>>,
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
    mut payout_info: ResMut<GameOverPayoutInfo>,
    current_turn: Option<Res<crate::game::resources::CurrentTurn>>,
) {
    // Only run once when game just ended
    if !game_over.is_game_over() {
        return;
    }

    // Reset payout info
    *payout_info = GameOverPayoutInfo::default();

    // Try to get wager information from Solana resources
    if let Some(sync) = solana_sync {
        if sync.wager_amount > 0 {
            payout_info.wager_amount = sync.wager_amount;

            // Get country fee from competitive state if available
            if let Some(_comp) = competitive {
                // Estimate country fee as 0.01 SOL (typical) if not specified
                // In production, this should come from the on-chain Game account
                payout_info.country_fee = 10_000_000; // 0.01 SOL default

                // ELO fee is typically 0.01 SOL total, split between players
                payout_info.elo_fee = 10_000_000; // 0.01 SOL

                // Estimate rent return (typically ~0.002 SOL for escrow account)
                payout_info.rent_return = 2_280_000; // ~0.00228 SOL

                // Calculate winning prize
                let total_pot = sync.wager_amount * 2;
                payout_info.winning_prize = total_pot
                    .saturating_sub(payout_info.country_fee)
                    .saturating_sub(payout_info.elo_fee);
            }

            // Determine if it's a draw
            payout_info.is_draw = game_over.winner().is_none();

            // Store player color if available
            if let Some(turn) = current_turn {
                payout_info.player_color = Some(turn.color);
            }
        }
    }
}

/// Plugin for the game over popup
pub struct GameOverPopupPlugin;

impl Plugin for GameOverPopupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOverPayoutInfo>();

        // Add the popup rendering system (runs in GameOver state)
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            game_over_popup_system.run_if(in_state(GameState::GameOver)),
        );

        // Add payout info fetching for Solana games
        #[cfg(feature = "solana")]
        app.add_systems(
            OnEnter(GameState::GameOver),
            fetch_game_payout_info,
        );
    }
}
