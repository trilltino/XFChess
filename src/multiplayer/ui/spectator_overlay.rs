//! Spectator HUD — shown at the bottom of the screen when `GameMode::Spectator`.
//!
//! Displays:
//! - "Spectating game {id}" label
//! - Live white/black clocks (interpolated locally between Braid broadcasts)
//! - A rolling chat log fed by `OnlineChatMessage` events

use crate::core::states::GameMode;
use crate::multiplayer::network::online_game_session::OnlineChatMessage;
use crate::multiplayer::spectator::{SpectatorClockState, SpectatorSession};
use crate::multiplayer::traits::MessageReader;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

const CHAT_MAX: usize = 8;

/// In-memory chat log; reset when spectator session changes.
#[derive(Resource, Default)]
pub struct SpectatorChatLog {
    pub messages: Vec<(String, String)>, // (player, text)
}

pub struct SpectatorOverlayPlugin;

impl Plugin for SpectatorOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorChatLog>().add_systems(
            Update,
            (drain_braid_chat_to_spectator_log, spectator_hud_system),
        );
    }
}

fn drain_braid_chat_to_spectator_log(
    mut chat_events: MessageReader<OnlineChatMessage>,
    mut log: ResMut<SpectatorChatLog>,
    game_mode: Res<GameMode>,
) {
    if *game_mode != GameMode::Spectator {
        return;
    }
    for msg in chat_events.read() {
        if log.messages.len() >= CHAT_MAX {
            log.messages.remove(0);
        }
        log.messages.push((msg.player.clone(), msg.text.clone()));
    }
}

pub fn spectator_hud_system(
    mut contexts: EguiContexts,
    game_mode: Res<GameMode>,
    session: Res<SpectatorSession>,
    clock: Res<SpectatorClockState>,
    chat_log: Res<SpectatorChatLog>,
) {
    if *game_mode != GameMode::Spectator {
        return;
    }
    let Some(ref game_id) = session.game_id else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::TopBottomPanel::bottom("spectator_hud")
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(20, 20, 20, 210),
            inner_margin: egui::Margin::symmetric(12, 6),
            ..egui::Frame::NONE
        })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left: "Spectating" label + proxy URL for web viewers
                ui.label(
                    egui::RichText::new(format!("Spectating game {}", game_id))
                        .size(12.0)
                        .color(egui::Color32::from_rgb(180, 180, 180))
                        .italics(),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "  •  http://localhost:8181/xfchess-game/{}/moves",
                        game_id
                    ))
                    .size(10.0)
                    .color(egui::Color32::from_rgb(100, 160, 100))
                    .monospace(),
                );

                ui.separator();

                // Centre: clocks
                let white_s = clock.white_ms / 1000;
                let black_s = clock.black_ms / 1000;
                let fmt_clock = |ms: u64| -> String {
                    let s = ms / 1000;
                    format!("{:02}:{:02}", s / 60, s % 60)
                };
                let white_color = if clock.white_to_move {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::GRAY
                };
                let black_color = if !clock.white_to_move {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::GRAY
                };
                ui.label(
                    egui::RichText::new(format!("⬜ {}", fmt_clock(clock.white_ms)))
                        .size(13.0)
                        .color(white_color)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(format!("⬛ {}", fmt_clock(clock.black_ms)))
                        .size(13.0)
                        .color(black_color)
                        .strong(),
                );

                // Suppress unused variable warnings when clocks are zero.
                let _ = white_s;
                let _ = black_s;
            });

            // Chat log (last N messages)
            if !chat_log.messages.is_empty() {
                ui.add_space(2.0);
                for (player, text) in &chat_log.messages {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}:", player))
                                .size(11.0)
                                .color(egui::Color32::from_rgb(120, 180, 255))
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new(text)
                                .size(11.0)
                                .color(egui::Color32::LIGHT_GRAY),
                        );
                    });
                }
            }
        });
}
