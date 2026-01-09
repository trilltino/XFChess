use crate::networking::client::MultiplayerSession;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use lightyear::prelude::*;
use shared::crdt::{MessageOpType, MessageOperation, MessageState};
use shared::protocol::{Channel1, GameMessage};
use uuid::Uuid;

// --- Resources ---

#[derive(Resource)]
pub struct ChatState {
    pub state: MessageState,
    pub current_input: String,
    pub is_visible: bool,
    pub local_user_id: Uuid, // Identity for CRDT
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            state: MessageState::new(Uuid::new_v4()),
            current_input: String::new(),
            is_visible: false,
            local_user_id: Uuid::new_v4(), // Should come from Auth
        }
    }
}

// --- Plugin ---

pub struct ChatUiPlugin;

impl Plugin for ChatUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatState>()
            .add_systems(Update, (toggle_chat_visibility, chat_ui_system));
    }
}

// Toggle chat visibility with 'C' key during multiplayer game
fn toggle_chat_visibility(
    keys: Res<ButtonInput<KeyCode>>,
    mut chat_state: ResMut<ChatState>,
    multiplayer_session: Option<Res<crate::networking::client::MultiplayerSession>>,
) {
    // Only show chat during active multiplayer game (not lobby)
    if let Some(session) = &multiplayer_session {
        if session.game_started && !chat_state.is_visible {
            chat_state.is_visible = true;
        }
        // Hide chat if game hasn't started
        if !session.game_started && chat_state.is_visible {
            chat_state.is_visible = false;
        }
    } else {
        // No multiplayer session means no chat
        chat_state.is_visible = false;
    }

    // Toggle with C key only during game
    if let Some(session) = &multiplayer_session {
        if session.game_started && keys.just_pressed(KeyCode::KeyC) {
            chat_state.is_visible = !chat_state.is_visible;
        }
    }
}

// --- Systems ---

fn chat_ui_system(
    mut contexts: EguiContexts,
    mut chat_state: ResMut<ChatState>,
    session: Res<MultiplayerSession>,
    mut sender_query: Query<&mut MessageSender<GameMessage>, With<Client>>,
) {
    if !chat_state.is_visible {
        return;
    }

    let ctx = contexts.ctx_mut();

    // Collect any message to delete (to avoid borrow conflict)
    let mut message_to_delete: Option<Uuid> = None;
    let local_user_id = chat_state.local_user_id;

    egui::Window::new("Chat")
        .default_width(300.0)
        .default_height(200.0)
        .show(ctx.expect("Egui context lost"), |ui| {
            // Message Area
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for msg in &chat_state.state.messages {
                        if !msg.is_deleted {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}: {}", msg.sender_id, msg.content));
                                // Delete button disabled until networking is re-enabled
                                if msg.sender_id == local_user_id {
                                    if ui.small_button("x").clicked() {
                                        message_to_delete = Some(msg.id);
                                    }
                                }
                            });
                        }
                    }
                });

            ui.separator();

            // Input Area
            ui.horizontal(|ui| {
                let res = ui.text_edit_singleline(&mut chat_state.current_input);
                if res.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !chat_state.current_input.trim().is_empty() {
                        send_message_local(&mut chat_state, &session, &mut sender_query);
                        res.request_focus();
                    }
                }

                if ui.button("Send").clicked() {
                    if !chat_state.current_input.trim().is_empty() {
                        send_message_local(&mut chat_state, &session, &mut sender_query);
                    }
                }
            });
        });

    // Apply delete after UI rendering to avoid borrow conflicts
    if let Some(msg_id) = message_to_delete {
        let timestamp = chat_state.state.current_timestamp.increment();
        let op = MessageOperation {
            op_type: MessageOpType::Delete,
            timestamp,
            message_id: msg_id,
            conversation_id: Uuid::nil(),
        };
        chat_state.state.apply(op);
    }
}

fn send_message_local(
    chat_state: &mut ResMut<ChatState>,
    session: &Res<MultiplayerSession>,
    sender_query: &mut Query<&mut MessageSender<GameMessage>, With<Client>>,
) {
    let content = chat_state.current_input.clone();
    let msg_id = Uuid::new_v4();
    let local_id = chat_state.local_user_id;

    let timestamp = chat_state.state.current_timestamp.increment();
    let op = MessageOperation {
        op_type: MessageOpType::Send {
            sender_id: local_id,
            content,
            message_type: "text".to_string(),
        },
        timestamp,
        message_id: msg_id,
        conversation_id: Uuid::nil(),
    };

    chat_state.state.apply(op.clone());
    chat_state.current_input.clear();

    // Send over network
    if let Some(entity) = session.client_entity {
        if let Ok(mut sender) = sender_query.get_mut(entity) {
            let _ = sender.send::<Channel1>(GameMessage::CrdtOperation(op));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_state_default() {
        let state = ChatState::default();

        assert!(state.current_input.is_empty());
        assert!(!state.is_visible);
        assert!(state.state.messages.is_empty());
    }

    #[test]
    fn test_chat_state_visibility_toggle() {
        let mut state = ChatState::default();

        assert!(!state.is_visible);
        state.is_visible = true;
        assert!(state.is_visible);
        state.is_visible = false;
        assert!(!state.is_visible);
    }

    #[test]
    fn test_chat_state_input_storage() {
        let mut state = ChatState::default();

        state.current_input = "Hello, world!".to_string();
        assert_eq!(state.current_input, "Hello, world!");

        state.current_input.clear();
        assert!(state.current_input.is_empty());
    }

    #[test]
    fn test_chat_state_user_id_uniqueness() {
        let state1 = ChatState::default();
        let state2 = ChatState::default();

        // Each instance should have a unique user ID
        assert_ne!(state1.local_user_id, state2.local_user_id);
    }
}
