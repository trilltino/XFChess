//! Spectator HUD — shown at the bottom of the screen when `GameMode::Spectator`.
//!
//! Displays:
//! - "Spectating game {id}" label
//! - Live white/black clocks (interpolated locally between Braid broadcasts)
//! - A rolling chat log fed by `OnlineChatMessage` events

use crate::core::states::{GameMode, GameState};
use crate::multiplayer::network::online_game_session::OnlineChatMessage;
use crate::multiplayer::spectator::{SpectatorClockState, SpectatorMatchInfo, SpectatorSession};
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
    mut game_mode: ResMut<GameMode>,
    mut session: ResMut<SpectatorSession>,
    match_info: Res<SpectatorMatchInfo>,
    clock: Res<SpectatorClockState>,
    mut chat_log: ResMut<SpectatorChatLog>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if *game_mode != GameMode::Spectator {
        return;
    }
    let Some(game_id) = session.game_id.clone() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let info = &match_info.0;
    let mut leave_clicked = false;

    egui::TopBottomPanel::bottom("spectator_hud")
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(20, 20, 20, 210),
            inner_margin: egui::Margin::symmetric(12, 6),
            ..egui::Frame::NONE
        })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left: matchup (player names when known) + tournament context.
                let title = match (&info.white, &info.black) {
                    (Some(w), Some(b)) => format!("{} vs {}", w, b),
                    (Some(w), None) => format!("Watching {}", w),
                    _ => format!("Spectating game {}", game_id),
                };
                ui.label(
                    egui::RichText::new(title)
                        .size(13.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                if let Some(ref tname) = info.tournament_name {
                    let round_str = info
                        .round
                        .map(|r| format!(" · Round {}", r + 1))
                        .unwrap_or_default();
                    ui.label(
                        egui::RichText::new(format!("{}{}", tname, round_str))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(255, 200, 100))
                            .italics(),
                    );
                }

                // Feed badge: delayed (anti-ghosting HTTP feed) vs live gossip.
                let (badge, badge_color) = if !session.delay_checked {
                    ("· CONNECTING", egui::Color32::GRAY)
                } else if session.delayed {
                    ("· DELAYED FEED", egui::Color32::from_rgb(220, 180, 60))
                } else {
                    ("· LIVE", egui::Color32::from_rgb(120, 220, 120))
                };
                ui.label(egui::RichText::new(badge).size(11.0).color(badge_color).strong());

                ui.separator();

                // Centre: clocks, labelled with names when known.
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
                let white_tag = info.white.as_deref().unwrap_or("White");
                let black_tag = info.black.as_deref().unwrap_or("Black");
                ui.label(
                    egui::RichText::new(format!("⬜ {} {}", white_tag, fmt_clock(clock.white_ms)))
                        .size(13.0)
                        .color(white_color)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(format!("⬛ {} {}", black_tag, fmt_clock(clock.black_ms)))
                        .size(13.0)
                        .color(black_color)
                        .strong(),
                );

                // Right: leave spectating.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Leave")
                                    .size(12.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(120, 50, 50))
                            .corner_radius(4.0)
                            .min_size(egui::vec2(60.0, 24.0)),
                        )
                        .clicked()
                    {
                        leave_clicked = true;
                    }
                });
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

    // Leave: tear down the spectator session and return to the main menu.
    if leave_clicked {
        session.game_id = None;
        session.pending_moves.clear();
        session.applied_move_count = 0;
        session.delay_result = None;
        chat_log.messages.clear();
        *game_mode = GameMode::SinglePlayer;
        next_state.set(GameState::MainMenu);
    }
}
