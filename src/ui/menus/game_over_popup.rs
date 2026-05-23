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
    /// Item 1: true once the on-chain finalization TX has confirmed.
    pub payout_confirmed: bool,
    /// Item 1: Solana transaction signature for the finalize_game TX.
    pub finalize_sig: Option<String>,
    /// Items 1+6: When the game ended (for dispute window timing).
    pub game_ended_at: Option<std::time::Instant>,
    /// Item 6: Pending dispute action — set when player clicks Dispute.
    pub dispute_pending: bool,
    /// Item 6: Dispute submitted confirmation message.
    pub dispute_sig: Option<String>,
    /// Wallet pubkey of the local player (for dispute signing).
    pub local_player_pubkey: Option<String>,
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
            self.wager_amount.saturating_sub(self.country_fee / 2).saturating_sub(self.elo_fee / 2)
        } else {
            self.winning_prize
        }
    }

    /// True if the dispute window (48 h) is still open.
    pub fn dispute_window_open(&self) -> bool {
        match self.game_ended_at {
            Some(t) => t.elapsed().as_secs() < 48 * 3600,
            None => false,
        }
    }
}

/// System to render the Game Over popup UI
pub fn game_over_popup_system(
    mut contexts: EguiContexts,
    game_over: Res<GameOverState>,
    payout_info: Option<Res<GameOverPayoutInfo>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let bg_color = egui::Color32::from_rgba_unmultiplied(10, 15, 20, 220);
    let accent_color = egui::Color32::from_rgb(244, 187, 68);
    let text_primary = egui::Color32::from_rgb(240, 240, 240);
    let text_secondary = egui::Color32::from_rgb(160, 160, 160);
    let text_gold = egui::Color32::from_rgb(244, 187, 68);
    let text_green = egui::Color32::from_rgb(80, 220, 120);
    let text_red = egui::Color32::from_rgb(255, 100, 100);

    let frame = egui::Frame::default()
        .fill(bg_color)
        .stroke(egui::Stroke::new(1.0, accent_color))
        .corner_radius(12.0)
        .inner_margin(20.0);

    let (winner_title, winner_color) = match game_over.winner() {
        Some(PieceColor::White) => ("White Wins!", text_primary),
        Some(PieceColor::Black) => ("Black Wins!", text_secondary),
        None => ("Draw!", accent_color),
    };

    let mut trigger_dispute = false;

    egui::Window::new("")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([480.0, 300.0])
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(440.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Game Over")
                        .size(20.0)
                        .strong()
                        .color(text_primary),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(winner_title)
                        .size(18.0)
                        .strong()
                        .color(winner_color),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(game_over.message())
                        .size(12.0)
                        .color(text_secondary),
                );

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

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Wager:").size(12.0).color(text_secondary));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(
                                    GameOverPayoutInfo::format_sol(info.wager_amount * 2)
                                ).size(12.0).color(text_primary));
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Treasury Fee:").size(12.0).color(text_secondary));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(
                                    format!("- {}", GameOverPayoutInfo::format_sol(info.country_fee))
                                ).size(12.0).color(text_red));
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("ELO Fee:").size(12.0).color(text_secondary));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(
                                    format!("- {}", GameOverPayoutInfo::format_sol(info.elo_fee))
                                ).size(12.0).color(text_red));
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Rent Returned:").size(12.0).color(text_secondary));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(
                                    format!("+ {}", GameOverPayoutInfo::format_sol(info.rent_return))
                                ).size(12.0).color(text_green));
                            });
                        });

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        let prize_text = if info.is_draw { "Returned:" } else { "You Won:" };
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(prize_text).size(14.0).strong().color(text_gold));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(
                                    GameOverPayoutInfo::format_sol(info.player_winnings())
                                ).size(16.0).strong().color(text_gold));
                            });
                        });

                        // Item 1: Prize claimed confirmation row.
                        ui.add_space(4.0);
                        if info.payout_confirmed {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Prize claimed ✓")
                                    .size(12.0).color(text_green));
                                if let Some(sig) = &info.finalize_sig {
                                    let short_sig = if sig.len() > 12 {
                                        format!("{}…", &sig[..8])
                                    } else {
                                        sig.clone()
                                    };
                                    let explorer_url = format!(
                                        "https://solscan.io/tx/{}?cluster=devnet", sig
                                    );
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.link(egui::RichText::new(short_sig)
                                            .size(11.0).color(text_secondary)).clicked()
                                        {
                                            #[cfg(target_os = "windows")]
                                            let _ = std::process::Command::new("cmd")
                                                .args(["/c", "start", "", &explorer_url])
                                                .spawn();
                                            #[cfg(target_os = "macos")]
                                            let _ = std::process::Command::new("open")
                                                .arg(&explorer_url)
                                                .spawn();
                                            #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
                                            let _ = std::process::Command::new("xdg-open")
                                                .arg(&explorer_url)
                                                .spawn();
                                        }
                                    });
                                }
                            });
                        } else {
                            ui.label(egui::RichText::new("Settling on-chain…")
                                .size(11.0).italics().color(text_secondary));
                        }

                        // Item 6: Dispute button — shown to the loser within 48 h.
                        let player_is_loser = match (game_over.winner(), info.player_color) {
                            (Some(PieceColor::White), Some(PieceColor::Black)) => true,
                            (Some(PieceColor::Black), Some(PieceColor::White)) => true,
                            _ => false,
                        };
                        if player_is_loser && info.payout_confirmed && info.dispute_window_open() {
                            ui.add_space(4.0);
                            if let Some(sig) = &info.dispute_sig {
                                ui.label(egui::RichText::new(
                                    format!("Dispute submitted — 48 h review window open ({})", &sig[..8.min(sig.len())])
                                ).size(11.0).color(text_secondary));
                            } else if ui.button(
                                egui::RichText::new("⚠ Dispute Result").size(12.0)
                            ).clicked() {
                                trigger_dispute = true;
                            }
                        }
                    }
                }

                ui.add_space(16.0);

                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;

                    let new_game_btn = ui.add_sized(
                        [150.0, 42.0],
                        egui::Button::new(egui::RichText::new("New Game").size(13.0).strong())
                            .fill(egui::Color32::from_rgb(60, 100, 60)),
                    );
                    if new_game_btn.clicked() {
                        info!("[GAME_OVER_POPUP] Starting new game");
                        next_state.set(GameState::InGame);
                    }

                    let main_menu_btn = ui.add_sized(
                        [150.0, 42.0],
                        egui::Button::new(egui::RichText::new("Main Menu").size(13.0).strong())
                            .fill(egui::Color32::from_rgb(80, 60, 60)),
                    );
                    if main_menu_btn.clicked() {
                        info!("[GAME_OVER_POPUP] Returning to main menu");
                        next_state.set(GameState::MainMenu);
                    }
                });
            });
        });

    // Trigger dispute outside the closure so we can call commands.
    if trigger_dispute {
        commands.insert_resource(PendingDispute {
            game_id: 0, // filled by apply_dispute_trigger system
        });
    }
}

/// Resource inserted when the player clicks "Dispute Result".
#[derive(Resource)]
pub struct PendingDispute {
    pub game_id: u64,
}

/// System to submit a pending dispute via VPS and update GameOverPayoutInfo.
pub fn apply_dispute_trigger(
    mut commands: Commands,
    dispute: Option<Res<PendingDispute>>,
    rollup: Option<Res<crate::multiplayer::rollup::manager::EphemeralRollupManager>>,
    payout_info: Option<ResMut<GameOverPayoutInfo>>,
) {
    let Some(dispute) = dispute else { return };
    let game_id = if let Some(mgr) = rollup {
        if mgr.game_id != 0 { mgr.game_id } else { dispute.game_id }
    } else {
        dispute.game_id
    };
    let Some(mut info) = payout_info else {
        commands.remove_resource::<PendingDispute>();
        return;
    };
    if info.dispute_pending {
        return;
    }
    info.dispute_pending = true;
    let player = info.local_player_pubkey.clone().unwrap_or_default();
    commands.remove_resource::<PendingDispute>();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            use crate::multiplayer::vps_client;
            match vps_client::vps_submit_dispute(game_id, &player) {
                Ok(sig) => info!("[DISPUTE] Submitted for game {} sig {}", game_id, sig),
                Err(e) => error!("[DISPUTE] Failed for game {}: {e}", game_id),
            }
        })
        .detach();
}

/// System to fetch on-chain payout information when game ends
#[cfg(feature = "solana")]
pub fn fetch_game_payout_info(
    game_over: Res<GameOverState>,
    solana_sync: Option<Res<crate::multiplayer::solana::addon::SolanaGameSync>>,
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
    mut payout_info: ResMut<GameOverPayoutInfo>,
    current_turn: Option<Res<crate::game::resources::CurrentTurn>>,
) {
    if !game_over.is_game_over() {
        return;
    }

    *payout_info = GameOverPayoutInfo::default();

    if let Some(sync) = solana_sync {
        if sync.wager_amount > 0 {
            payout_info.wager_amount = sync.wager_amount;

            if let Some(_comp) = competitive {
                payout_info.country_fee = 10_000_000;
                payout_info.elo_fee = 10_000_000;
                payout_info.rent_return = 2_280_000;
                let total_pot = sync.wager_amount * 2;
                payout_info.winning_prize = total_pot
                    .saturating_sub(payout_info.country_fee)
                    .saturating_sub(payout_info.elo_fee);
            }

            payout_info.is_draw = game_over.winner().is_none();

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

        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            game_over_popup_system.run_if(in_state(GameState::GameOver)),
        );

        app.add_systems(Update, apply_dispute_trigger);

        #[cfg(feature = "solana")]
        app.add_systems(OnEnter(GameState::GameOver), fetch_game_payout_info);
    }
}
