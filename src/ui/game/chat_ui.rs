//! In-game P2P chat panel — rendered as a collapsible egui side panel.
//!
//! Systems:
//!  - `drain_chat_messages`  — drains `BraidPvpSession.chat_rx` → `ChatState.history` each frame
//!  - `chat_panel_ui`        — renders the egui floating panel; fires `PublishBraidChat` on send

use bevy::prelude::*;
use bevy_egui::egui;
use braid_uri::ChatPayload;
use crossbeam_channel::TryRecvError;

use crate::multiplayer::network::{BraidPvpSession, PublishBraidChat};
use crate::core::states::GameMode;

// ── State ─────────────────────────────────────────────────────────────────────

/// One received or sent chat line.
#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
}

/// Bevy resource tracking all chat history + compose state for the active session.
#[derive(Resource, Default)]
pub struct ChatState {
    pub history: Vec<ChatEntry>,
    /// Current text in the compose box.
    pub input: String,
    /// Whether the panel is expanded (visible).
    pub open: bool,
    /// Unread badge count (increments when panel is closed).
    pub unread: usize,
}

impl ChatState {
    pub fn push_inbound(&mut self, payload: ChatPayload) {
        self.history.push(ChatEntry {
            player: payload.player,
            text: payload.text,
            timestamp_ms: payload.timestamp_ms,
        });
        if !self.open {
            self.unread += 1;
        }
    }

    pub fn reset(&mut self) {
        self.history.clear();
        self.input.clear();
        self.unread = 0;
    }
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// Drain inbound chat messages from the Braid background subscriber task into `ChatState`.
pub fn drain_chat_messages(
    session: Option<Res<BraidPvpSession>>,
    mut chat_state: ResMut<ChatState>,
) {
    let Some(session) = session else { return };
    if !session.active {
        return;
    }
    loop {
        match session.chat_rx.try_recv() {
            Ok(payload) => chat_state.push_inbound(payload),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

/// Render the chat panel overlay. Fires `PublishBraidChat` when the user submits.
pub fn chat_panel_ui(
    mut contexts: bevy_egui::EguiContexts,
    session: Option<Res<BraidPvpSession>>,
    mut chat_state: ResMut<ChatState>,
    game_mode: Res<GameMode>,
    player_name: Option<Res<crate::states::main_menu::PlayerIdentity>>,
    mut chat_writer: MessageWriter<PublishBraidChat>,
) {
    // Only render during P2P/Braid multiplayer sessions.
    if !matches!(
        *game_mode,
        GameMode::BraidMultiplayer | GameMode::MultiplayerCompetitive
    ) {
        return;
    }
    let Some(session) = session else { return };
    if !session.active {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    // ── Toggle button (bottom-right) ──────────────────────────────────────────
    egui::Area::new(egui::Id::new("chat_toggle"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
        .show(ctx, |ui| {
            let label = if chat_state.unread > 0 {
                format!("💬 Chat ({})", chat_state.unread)
            } else {
                "💬 Chat".to_string()
            };
            let btn = egui::Button::new(
                egui::RichText::new(label)
                    .size(13.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .fill(egui::Color32::from_rgb(30, 30, 50))
            .corner_radius(6.0);
            if ui.add_sized([110.0, 30.0], btn).clicked() {
                chat_state.open = !chat_state.open;
                if chat_state.open {
                    chat_state.unread = 0;
                }
            }
        });

    if !chat_state.open {
        return;
    }

    // ── Chat panel ────────────────────────────────────────────────────────────
    let panel_width = 280.0;
    let panel_height = 340.0;

    egui::Window::new("Chat")
        .id(egui::Id::new("chat_panel"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -50.0))
        .fixed_size([panel_width, panel_height])
        .collapsible(false)
        .title_bar(false)
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(15, 15, 25, 230))
                .corner_radius(8.0)
                .inner_margin(8.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 90))),
        )
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Chat")
                        .size(14.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").clicked() {
                        chat_state.open = false;
                    }
                });
            });
            ui.separator();

            // History scroll area
            let history_height = panel_height - 90.0;
            egui::ScrollArea::vertical()
                .id_salt("chat_history")
                .max_height(history_height)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    // Clone to avoid borrow conflict
                    let entries: Vec<_> = chat_state.history.iter().cloned().collect();
                    for entry in &entries {
                        let name_color = if entry.player.contains("white") || entry.player.contains("host") {
                            egui::Color32::from_rgb(180, 200, 255)
                        } else {
                            egui::Color32::from_rgb(255, 200, 150)
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{}:", entry.player))
                                    .size(11.0)
                                    .color(name_color)
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new(&entry.text)
                                    .size(12.0)
                                    .color(egui::Color32::WHITE),
                            );
                        });
                    }
                    if entries.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("No messages yet.")
                                    .size(12.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    }
                });

            ui.add_space(4.0);
            ui.separator();

            // Compose row
            ui.horizontal(|ui| {
                let input_width = panel_width - 60.0;
                let response = ui.add(
                    egui::TextEdit::singleline(&mut chat_state.input)
                        .hint_text("Say something...")
                        .desired_width(input_width)
                        .font(egui::TextStyle::Body),
                );

                let can_send = !chat_state.input.trim().is_empty()
                    && chat_state.input.len() <= 500;

                let send_pressed = response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));

                let btn_clicked = ui
                    .add_enabled(
                        can_send,
                        egui::Button::new(
                            egui::RichText::new("Send")
                                .size(12.0)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(40, 120, 60))
                        .corner_radius(4.0),
                    )
                    .clicked();

                if (send_pressed || btn_clicked) && can_send {
                    let text = chat_state.input.trim().to_string();
                    let player = player_name
                        .as_ref()
                        .map(|p| p.display_name().to_string())
                        .unwrap_or_else(|| "me".to_string());
                    let timestamp_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    // Optimistically append to own history.
                    chat_state.history.push(ChatEntry {
                        player: player.clone(),
                        text: text.clone(),
                        timestamp_ms,
                    });

                    chat_writer.write(PublishBraidChat {
                        player,
                        text,
                        timestamp_ms,
                    });

                    chat_state.input.clear();
                    response.request_focus();
                }
            });
        });
}
