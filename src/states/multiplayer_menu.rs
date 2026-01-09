use crate::core::GameState;
use crate::networking::client::{
    connect_to_server, send_lobby_message, LobbyScreen, LobbyUiState, MultiplayerSession,
};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use lightyear::prelude::*;
use shared::protocol::LobbyMessage;

pub struct MultiplayerMenuPlugin;

impl Plugin for MultiplayerMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MultiplayerMenu), reset_lobby_state)
            .add_systems(
                EguiPrimaryContextPass,
                multiplayer_menu_ui.run_if(in_state(GameState::MultiplayerMenu)),
            );
    }
}

fn reset_lobby_state(mut session: ResMut<MultiplayerSession>, mut lobby_ui: ResMut<LobbyUiState>) {
    *session = MultiplayerSession::default();
    *lobby_ui = LobbyUiState::default();
}

fn multiplayer_menu_ui(
    mut contexts: EguiContexts,
    mut commands: Commands,
    mut session: ResMut<MultiplayerSession>,
    mut lobby_ui: ResMut<LobbyUiState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut lobby_senders: Query<&mut MessageSender<LobbyMessage>, With<Client>>,
) {
    let Some(ctx) = contexts.ctx_mut().ok() else {
        return;
    };

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(egui::Color32::from_rgb(20, 20, 25)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);

                match lobby_ui.screen {
                    LobbyScreen::CreateJoin => {
                        show_create_join_screen(ui, &mut commands, &mut session, &mut lobby_ui);
                    }
                    LobbyScreen::Waiting => {
                        show_waiting_screen(ui, &mut session, &mut lobby_ui, &mut lobby_senders);
                    }
                }

                ui.add_space(30.0);

                // Error message
                if !lobby_ui.error_message.is_empty() {
                    ui.label(
                        egui::RichText::new(&lobby_ui.error_message)
                            .color(egui::Color32::RED)
                            .size(14.0),
                    );
                }

                ui.add_space(20.0);

                // Back button
                if ui.button("Back to Menu").clicked() {
                    next_state.set(GameState::MainMenu);
                }
            });
        });
}

fn show_create_join_screen(
    ui: &mut egui::Ui,
    commands: &mut Commands,
    session: &mut ResMut<MultiplayerSession>,
    lobby_ui: &mut ResMut<LobbyUiState>,
) {
    ui.label(
        egui::RichText::new("Multiplayer")
            .size(32.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );

    ui.add_space(40.0);

    // Create Room
    ui.label(egui::RichText::new("Host a Game").color(egui::Color32::LIGHT_GRAY));
    ui.add_space(10.0);

    if ui.button("Create Room").clicked() {
        // Generate random code client-side
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::rng(); // Use `rng()` per deprecation warning, or `thread_rng()` if that was the warning?
                                   // Wait, warning said `thread_rng` renamed to `rng`? Or `rng` renamed to `thread_rng`?
                                   // Warning said: "use of deprecated function `rand::thread_rng`: Renamed to `rng`"
                                   // So use `rand::rng()`.
                                   // But `rand` version 0.9?
                                   // Let's use `rand::thread_rng()` and suppress warning or just use `rand::rng()`.
                                   // I'll stick to `rand::rng()` if available, else `thread_rng`.
                                   // Actually simplest is just simple `rand` usage if possible.
                                   // Let's assume `rand::rng()` works based on warning.

        // Actually, just use `rand::random` char? No.

        let code: String = (0..6)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len()); // Warning said `gen_range` renamed to `random_range`
                CHARSET[idx] as char
            })
            .collect();

        let entity = connect_to_server(commands);
        session.client_entity = Some(entity);
        session.is_host = true;
        // session.pending_create = true; // No longer needed
        session.pending_room_code = code.clone(); // Implicit create via Join
        session.room_code = code; // Optimistically set room code so UI helps debugging? No, wait for RoomCreated.
    }

    ui.add_space(10.0);
    if session.is_host && !session.is_connected {
        ui.label(egui::RichText::new("Connecting...").color(egui::Color32::YELLOW));
    }

    ui.add_space(30.0);
    ui.separator();
    ui.add_space(30.0);

    // Join Room
    ui.label(egui::RichText::new("Join a Game").color(egui::Color32::LIGHT_GRAY));
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("Room Code:");
        ui.text_edit_singleline(&mut lobby_ui.room_code_input);
    });

    ui.add_space(10.0);

    if ui.button("Join Room").clicked() && !lobby_ui.room_code_input.is_empty() {
        let entity = connect_to_server(commands);
        session.client_entity = Some(entity);
        session.is_host = false;
        session.pending_room_code = lobby_ui.room_code_input.clone(); // Will send JoinRoom after connection
    }

    if !session.is_host && !session.pending_room_code.is_empty() && !session.is_connected {
        ui.label(egui::RichText::new("Connecting...").color(egui::Color32::YELLOW));
    }

    ui.add_space(20.0);
    ui.label(
        egui::RichText::new(format!(
            "Debug: Connected={}, Host={}, PendingCreate={}, RoomCode='{}'",
            session.is_connected, session.is_host, session.pending_create, session.room_code
        ))
        .size(10.0)
        .color(egui::Color32::DARK_GRAY),
    );
}

fn show_waiting_screen(
    ui: &mut egui::Ui,
    session: &mut ResMut<MultiplayerSession>,
    lobby_ui: &mut ResMut<LobbyUiState>,
    lobby_senders: &mut Query<&mut MessageSender<LobbyMessage>, With<Client>>,
) {
    ui.label(
        egui::RichText::new("Lobby")
            .size(32.0)
            .color(egui::Color32::WHITE)
            .strong(),
    );

    ui.add_space(20.0);

    // Room code display
    ui.label(
        egui::RichText::new(format!("Room Code: {}", session.room_code))
            .size(24.0)
            .color(egui::Color32::YELLOW)
            .strong(),
    );

    ui.add_space(20.0);

    // Player status
    let you_label = if session.is_host {
        "You (Host - White)"
    } else {
        "You (Guest - Black)"
    };
    let you_ready = if session.is_host {
        session.host_ready
    } else {
        session.guest_ready
    };

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(you_label).color(egui::Color32::WHITE));
        if you_ready {
            ui.label(egui::RichText::new("✓ READY").color(egui::Color32::GREEN));
        }
    });

    if session.is_host {
        let opponent_status = if session.opponent_joined {
            if session.guest_ready {
                "Opponent: ✓ READY"
            } else {
                "Opponent: Waiting..."
            }
        } else {
            "Waiting for opponent..."
        };
        ui.label(egui::RichText::new(opponent_status).color(egui::Color32::LIGHT_GRAY));
    } else {
        let host_status = if session.host_ready {
            "Host: ✓ READY"
        } else {
            "Host: Waiting..."
        };
        ui.label(egui::RichText::new(host_status).color(egui::Color32::LIGHT_GRAY));
    }

    ui.add_space(30.0);

    // Ready button
    let my_ready = if session.is_host {
        session.host_ready
    } else {
        session.guest_ready
    };
    let ready_text = if my_ready { "Cancel Ready" } else { "Ready Up" };

    if ui.button(ready_text).clicked() {
        let new_ready = !my_ready;
        if session.is_host {
            session.host_ready = new_ready;
        } else {
            session.guest_ready = new_ready;
        }
        send_lobby_message(
            &session,
            LobbyMessage::SetReady { ready: new_ready },
            lobby_senders,
        );
    }

    ui.add_space(20.0);

    // Start button (host only)
    if session.is_host {
        let can_start = session.host_ready && session.guest_ready && session.opponent_joined;

        ui.add_enabled_ui(can_start, |ui| {
            if ui.button("Start Game").clicked() {
                send_lobby_message(&session, LobbyMessage::StartGame, lobby_senders);
            }
        });

        if !can_start {
            ui.label(
                egui::RichText::new("Both players must be ready to start")
                    .size(12.0)
                    .color(egui::Color32::GRAY),
            );
        }
    }
}
