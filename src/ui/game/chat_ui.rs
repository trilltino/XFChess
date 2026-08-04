//! In-game online chat — state + inline rendering for the left game-info panel.
//!
//! Systems:
//!  - `drain_chat_messages` - drains `OnlineChatMessage` events into `ChatState.history`
//!
//! Rendering happens inline inside `crate::ui::game::left_panel::render_game_left_panel`
//! via `render_chat_section`, not as a standalone floating window.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::multiplayer::network::{OnlineChatMessage, OnlineGameSession, PublishOnlineChat};
use crate::multiplayer::traits::MessageReader;

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
}

impl ChatState {
    pub fn push_inbound(&mut self, player: String, text: String, timestamp_ms: u64) {
        self.history.push(ChatEntry {
            player,
            text,
            timestamp_ms,
        });
    }

    pub fn reset(&mut self) {
        self.history.clear();
        self.input.clear();
    }
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// Drain inbound `OnlineChatMessage` events into `ChatState`.
pub fn drain_chat_messages(
    session: Option<Res<OnlineGameSession>>,
    mut chat_events: MessageReader<OnlineChatMessage>,
    mut chat_state: ResMut<ChatState>,
) {
    let Some(session) = session else {
        chat_events.clear();
        return;
    };
    if !session.active {
        chat_events.clear();
        return;
    }
    for msg in chat_events.read() {
        chat_state.push_inbound(msg.player.clone(), msg.text.clone(), msg.timestamp_ms);
    }
}

/// Render the chat history + compose row inline inside a panel section.
/// `max_height` bounds the scrollable history area. Fires `PublishOnlineChat`
/// on send (and optimistically appends the message to local history).
pub fn render_chat_section(
    ui: &mut egui::Ui,
    chat_state: &mut ChatState,
    chat_writer: &mut MessageWriter<PublishOnlineChat>,
    player_display_name: &str,
    max_height: f32,
) {
    ui.label(
        egui::RichText::new("Chat")
            .size(12.0)
            .strong()
            .color(crate::ui::styles::UiColors::TEXT_SECONDARY),
    );
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .id_salt("left_panel_chat_history")
        .max_height(max_height)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if chat_state.history.is_empty() {
                ui.label(
                    egui::RichText::new("No messages yet.")
                        .size(11.5)
                        .color(egui::Color32::GRAY),
                );
            }
            let entries: Vec<_> = chat_state.history.iter().cloned().collect();
            for entry in &entries {
                let name_color = if entry.player.contains("white") || entry.player.contains("host")
                {
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
                            .size(11.5)
                            .color(egui::Color32::WHITE),
                    );
                });
            }
        });

    ui.add_space(4.0);

    ui.horizontal(|ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut chat_state.input)
                .hint_text("Say something...")
                .desired_width(ui.available_width() - 50.0)
                .font(egui::TextStyle::Body),
        );

        let can_send = !chat_state.input.trim().is_empty() && chat_state.input.len() <= 500;

        let send_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let btn_clicked = ui
            .add_enabled(
                can_send,
                egui::Button::new(
                    egui::RichText::new("Send")
                        .size(11.5)
                        .color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(40, 120, 60))
                .corner_radius(4.0),
            )
            .clicked();

        if (send_pressed || btn_clicked) && can_send {
            let text = chat_state.input.trim().to_string();
            let player = player_display_name.to_string();
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

            chat_writer.write(PublishOnlineChat {
                player,
                text,
                timestamp_ms,
            });

            chat_state.input.clear();
            response.request_focus();
        }
    });
}
