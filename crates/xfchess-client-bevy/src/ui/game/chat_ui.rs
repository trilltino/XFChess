use bevy::prelude::*;
use bevy_egui::egui;

use crate::multiplayer::network::{OnlineChatMessage, OnlineGameSession, PublishOnlineChat};
use crate::multiplayer::traits::MessageReader;

#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
    /// Typed on this device. A line that comes back from the log after a restart is
    /// not marked; [`is_mine`] recognises it by name instead.
    pub mine: bool,
}

/// Oldest lines are dropped past this, so a long game or a replayed log stays bounded.
const MAX_HISTORY: usize = 200;

#[derive(Resource, Default)]
pub struct ChatState {
    pub history: Vec<ChatEntry>,
    pub input: String,
    /// Game this transcript belongs to; a different game starts a fresh one.
    game_id: Option<String>,
}

impl ChatState {
    /// Add a line unless it is already shown. One message reaches us several ways
    /// (our own optimistic echo, gossip, the Braid stream, a replay after a
    /// reconnect), and all of them carry the same sender, text and timestamp.
    fn push(&mut self, entry: ChatEntry) -> bool {
        let seen = self.history.iter().any(|known| {
            known.timestamp_ms == entry.timestamp_ms
                && known.player == entry.player
                && known.text == entry.text
        });
        if seen {
            return false;
        }
        self.history.push(entry);
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        true
    }

    pub fn push_inbound(&mut self, player: String, text: String, timestamp_ms: u64) -> bool {
        self.push(ChatEntry {
            player,
            text,
            timestamp_ms,
            mine: false,
        })
    }

    /// Show our own line immediately. Returns false when the identical line is
    /// already there, in which case it must not be sent a second time.
    pub fn push_local(&mut self, player: String, text: String, timestamp_ms: u64) -> bool {
        self.push(ChatEntry {
            player,
            text,
            timestamp_ms,
            mine: true,
        })
    }

    /// Drop the transcript when the session moved on to a different game.
    pub fn scope_to_game(&mut self, game_id: &str) {
        if self.game_id.as_deref() != Some(game_id) {
            self.history.clear();
            self.game_id = Some(game_id.to_string());
        }
    }

    pub fn reset(&mut self) {
        self.history.clear();
        self.input.clear();
        self.game_id = None;
    }
}

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
    chat_state.scope_to_game(&session.game_id);
    for msg in chat_events.read() {
        chat_state.push_inbound(msg.player.clone(), msg.text.clone(), msg.timestamp_ms);
    }
}

/// Whether a line is ours. Lines typed here are marked; lines replayed from the log
/// after a restart are recognised by our name, unless the opponent shares it (then
/// they cannot be told apart and count as the opponent's).
fn is_mine(entry: &ChatEntry, me: &str, opponent: Option<&str>) -> bool {
    entry.mine || (entry.player == me && opponent != Some(me))
}

const MINE: egui::Color32 = egui::Color32::from_rgb(130, 190, 255);
const THEIRS: egui::Color32 = egui::Color32::from_rgb(255, 190, 120);

/// The chat card body: title on top, the conversation filling the middle with the
/// newest line at the bottom (as in any chat app), the input underneath.
pub fn render_chat_section(
    ui: &mut egui::Ui,
    chat_state: &mut ChatState,
    chat_writer: &mut MessageWriter<PublishOnlineChat>,
    player_display_name: &str,
    opponent_display_name: Option<&str>,
    max_height: f32,
) {
    ui.label(
        egui::RichText::new("Chat")
            .size(12.0)
            .strong()
            .color(crate::ui::styles::UiColors::TEXT_SECONDARY),
    );
    ui.add_space(4.0);

    // egui lays a scroll area out from the top, so a short conversation is pushed down
    // by the height it leaves empty (measured the frame before).
    let height_id = ui.id().with("chat_content_height");
    let last_height: f32 = ui.data(|data| data.get_temp(height_id)).unwrap_or(0.0);
    egui::ScrollArea::vertical()
        .id_salt("left_panel_chat_history")
        .max_height(max_height)
        .min_scrolled_height(max_height)
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.add_space((max_height - last_height).max(0.0));
            let top = ui.cursor().top();
            if chat_state.history.is_empty() {
                ui.label(
                    egui::RichText::new("No messages yet.")
                        .size(11.5)
                        .color(egui::Color32::GRAY),
                );
            }
            for entry in &chat_state.history {
                let mine = is_mine(entry, player_display_name, opponent_display_name);
                let (name, color) = if mine {
                    ("You", MINE)
                } else {
                    (entry.player.as_str(), THEIRS)
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{name}:"))
                            .size(11.0)
                            .color(color)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(&entry.text)
                            .size(11.5)
                            .color(egui::Color32::WHITE),
                    );
                });
            }
            let content = ui.cursor().top() - top;
            ui.data_mut(|data| data.insert_temp(height_id, content));
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

            // Show it now; the echo of our own message coming back is deduplicated.
            if chat_state.push_local(player.clone(), text.clone(), timestamp_ms) {
                chat_writer.write(PublishOnlineChat {
                    player,
                    text,
                    timestamp_ms,
                });
            }

            chat_state.input.clear();
            response.request_focus();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_message_arriving_twice_is_shown_once() {
        let mut chat = ChatState::default();
        assert!(chat.push_local("tino".into(), "hi".into(), 10));
        // Echo over gossip, then over the Braid stream or a replay after reconnect.
        assert!(!chat.push_inbound("tino".into(), "hi".into(), 10));
        assert!(!chat.push_inbound("tino".into(), "hi".into(), 10));
        assert_eq!(chat.history.len(), 1);
    }

    #[test]
    fn lines_are_ours_by_origin_or_by_a_name_the_opponent_does_not_share() {
        let typed_here = ChatEntry {
            player: "tino".into(),
            text: "hi".into(),
            timestamp_ms: 1,
            mine: true,
        };
        let replayed = ChatEntry {
            mine: false,
            ..typed_here.clone()
        };
        assert!(is_mine(&typed_here, "tino", Some("tino")));
        assert!(
            is_mine(&replayed, "tino", Some("mika")),
            "recognised by name after a restart"
        );
        assert!(
            !is_mine(&replayed, "tino", Some("tino")),
            "same name: cannot tell"
        );
        assert!(!is_mine(&replayed, "mika", Some("tino")));
    }

    #[test]
    fn only_typed_lines_are_marked_mine() {
        let mut chat = ChatState::default();
        chat.push_local("a".into(), "x".into(), 1);
        chat.push_inbound("b".into(), "y".into(), 2);
        assert!(chat.history[0].mine && !chat.history[1].mine);
    }

    #[test]
    fn repeated_text_at_a_different_time_is_a_new_message() {
        let mut chat = ChatState::default();
        chat.push_inbound("tino".into(), "hi".into(), 10);
        assert!(chat.push_inbound("tino".into(), "hi".into(), 11));
        assert_eq!(chat.history.len(), 2);
    }

    #[test]
    fn a_duplicate_local_line_is_not_sent_again() {
        let mut chat = ChatState::default();
        assert!(chat.push_local("tino".into(), "hi".into(), 10));
        assert!(!chat.push_local("tino".into(), "hi".into(), 10));
    }

    #[test]
    fn a_new_game_starts_an_empty_transcript() {
        let mut chat = ChatState::default();
        chat.scope_to_game("1");
        chat.push_inbound("tino".into(), "hi".into(), 10);
        chat.scope_to_game("1");
        assert_eq!(chat.history.len(), 1);
        chat.scope_to_game("2");
        assert!(chat.history.is_empty());
    }

    #[test]
    fn history_is_bounded() {
        let mut chat = ChatState::default();
        for i in 0..(MAX_HISTORY as u64 + 20) {
            chat.push_inbound("tino".into(), "x".into(), i);
        }
        assert_eq!(chat.history.len(), MAX_HISTORY);
        assert_eq!(chat.history[0].timestamp_ms, 20);
    }
}
