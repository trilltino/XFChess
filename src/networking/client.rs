use crate::core::GameState;
use bevy::prelude::*;
use std::net::{Ipv4Addr, SocketAddr};

use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use shared::protocol::{Channel1, GameMessage, LobbyMessage};

/// Multiplayer session state
#[derive(Resource, Default, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct MultiplayerSession {
    pub is_active: bool,
    pub is_connected: bool,
    pub is_host: bool, // Host=White, Joiner=Black
    pub room_code: String,
    pub pending_room_code: String, // Code to join after connection
    pub pending_create: bool,      // Want to create room after connection
    pub opponent_joined: bool,
    pub host_ready: bool,
    pub guest_ready: bool,
    pub game_started: bool,
    pub player_color_white: bool, // True if this player is White
    pub client_entity: Option<Entity>,
}

/// State of the lobby UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LobbyScreen {
    #[default]
    CreateJoin, // Initial: Create or Join room
    Waiting, // In lobby waiting for opponent/ready
}

#[derive(Resource, Default)]
pub struct LobbyUiState {
    pub screen: LobbyScreen,
    pub room_code_input: String,
    pub error_message: String,
    pub chat_messages: Vec<(String, String)>, // (Sender, Content)
}

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MultiplayerSession>();
        app.init_resource::<LobbyUiState>();
        app.register_type::<MultiplayerSession>();

        app.add_plugins(ClientPlugins {
            tick_duration: std::time::Duration::from_secs_f64(1.0 / 60.0),
        });

        app.add_plugins(shared::protocol::ProtocolPlugin);

        app.add_systems(
            Update,
            (
                handle_connection_events,
                handle_lobby_messages,
                handle_game_messages,
            )
                .run_if(resource_exists::<MultiplayerSession>),
        );

        info!("NetworkingPlugin loaded");
    }
}

/// Connect to the backend server
pub fn connect_to_server(commands: &mut Commands) -> Entity {
    let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 5000);
    let client_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);

    info!("[NETWORK] Connecting to server at {}", server_addr);

    let auth = Authentication::Manual {
        server_addr,
        client_id: rand::random::<u64>(),
        private_key: Key::default(),
        protocol_id: 0,
    };

    let netcode_config = NetcodeConfig {
        client_timeout_secs: 10,
        token_expire_secs: -1,
        ..default()
    };

    let client_entity = commands
        .spawn((
            Client::default(),
            LocalAddr(client_addr),
            PeerAddr(server_addr),
            Link::new(None),
            ReplicationReceiver::default(),
            PredictionManager::default(),
            NetcodeClient::new(auth, netcode_config).expect("Failed to create NetcodeClient"),
            UdpIo::default(),
            Name::new("GameClient"),
            MessageSender::<LobbyMessage>::default(),
            MessageReceiver::<LobbyMessage>::default(),
            MessageSender::<GameMessage>::default(),
            MessageReceiver::<GameMessage>::default(),
        ))
        .id();

    commands.trigger(Connect {
        entity: client_entity,
    });

    info!("[NETWORK] Client entity spawned: {:?}", client_entity);
    client_entity
}

/// Send a lobby message to server
pub fn send_lobby_message(
    session: &MultiplayerSession,
    message: LobbyMessage,
    sender_query: &mut Query<&mut MessageSender<LobbyMessage>, With<Client>>,
) {
    if let Some(entity) = session.client_entity {
        if let Ok(mut sender) = sender_query.get_mut(entity) {
            let _ = sender.send::<Channel1>(message);
        }
    }
}

fn handle_connection_events(
    mut session: ResMut<MultiplayerSession>,
    newly_connected: Query<Entity, Added<Connected>>,
    disconnected: Query<Entity, Added<Disconnected>>,
    connected_clients: Query<Entity, (With<Client>, With<Connected>)>,
    mut lobby_senders: Query<&mut MessageSender<LobbyMessage>, With<Client>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // 1. Handle new connections
    for entity in newly_connected.iter() {
        info!("[NETWORK] Connected to server! Entity: {:?}", entity);
        session.is_connected = true;
        session.client_entity = Some(entity);
    }

    // 2. Handle disconnections
    for _ in disconnected.iter() {
        warn!("[NETWORK] Disconnected from server!");
        session.is_connected = false;
        session.is_active = false;
        session.game_started = false;
        session.room_code.clear();
        session.pending_create = false;
        session.pending_room_code.clear();
        session.client_entity = None;

        // Auto-transition back to menu to avoid being stuck in game
        next_state.set(GameState::MultiplayerMenu);
    }

    // 3. Process pending actions (retry until sent)
    if let Some(entity) = session.client_entity {
        if connected_clients.contains(entity) {
            // Ensure session thinks we are connected
            if !session.is_connected {
                session.is_connected = true;
            }

            if session.pending_create {
                if let Ok(mut sender) = lobby_senders.get_mut(entity) {
                    info!("[NETWORK] Sending CreateRoom (Pending)");
                    let _ = sender.send::<Channel1>(LobbyMessage::CreateRoom);
                    session.pending_create = false;
                }
            } else if !session.pending_room_code.is_empty() {
                if let Ok(mut sender) = lobby_senders.get_mut(entity) {
                    let code = session.pending_room_code.clone();
                    info!("[NETWORK] Sending JoinRoom: {} (Pending)", code);
                    let _ = sender.send::<Channel1>(LobbyMessage::JoinRoom { code });
                    session.pending_room_code.clear();
                } else {
                    warn!(
                        "[NETWORK] Client entity missing MessageSender<LobbyMessage> in JoinRoom!"
                    );
                }
            }
        }
    }
}

fn handle_lobby_messages(
    mut query: Query<&mut MessageReceiver<LobbyMessage>, With<Client>>,
    mut session: ResMut<MultiplayerSession>,
    mut lobby_ui: ResMut<LobbyUiState>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for mut receiver in query.iter_mut() {
        for msg in receiver.receive() {
            match msg {
                LobbyMessage::RoomCreated { code } => {
                    info!("[LOBBY] Room created: {}", code);
                    session.room_code = code;
                    if session.is_host {
                        lobby_ui.screen = LobbyScreen::Waiting;
                        lobby_ui.error_message.clear();
                    }
                }
                LobbyMessage::JoinedRoom { code, is_host } => {
                    info!(
                        "[LOBBY] Joined room {} as {}",
                        code,
                        if is_host { "host" } else { "guest" }
                    );
                    session.room_code = code;
                    session.is_host = is_host;
                    session.is_active = true;
                    lobby_ui.screen = LobbyScreen::Waiting;
                    lobby_ui.error_message.clear();
                }
                LobbyMessage::PlayerJoined { player_id } => {
                    info!("[LOBBY] Opponent joined: {}", player_id);
                    session.opponent_joined = true;
                }
                LobbyMessage::PlayerLeft { player_id } => {
                    info!("[LOBBY] Opponent left: {}", player_id);
                    session.opponent_joined = false;
                    session.guest_ready = false;
                }
                LobbyMessage::PlayerReady {
                    player_id: _,
                    ready,
                } => {
                    // Update ready states (simplified - assumes other player)
                    if session.is_host {
                        session.guest_ready = ready;
                    } else {
                        session.host_ready = ready;
                    }
                }
                LobbyMessage::GameStarting { your_color } => {
                    info!(
                        "[LOBBY] Game starting! You are {}",
                        if your_color { "White" } else { "Black" }
                    );
                    session.game_started = true;
                    session.player_color_white = your_color;
                    next_state.set(GameState::InGame);
                }
                LobbyMessage::Error { message } => {
                    warn!("[LOBBY] Error: {}", message);
                    lobby_ui.error_message = message;
                }
                _ => {}
            }
        }
    }
}

fn handle_game_messages(
    mut query: Query<&mut MessageReceiver<GameMessage>, With<Client>>,
    mut lobby_ui: ResMut<LobbyUiState>,
    mut chat_state: ResMut<crate::ui::chat::ChatState>,
    mut network_move_events: MessageWriter<crate::game::events::NetworkMoveEvent>,
) {
    for mut receiver in query.iter_mut() {
        for msg in receiver.receive() {
            match msg {
                GameMessage::MoveMade { from, to } => {
                    info!("[GAME] Net Move: {:?} -> {:?}", from, to);
                    network_move_events.write(crate::game::events::NetworkMoveEvent { from, to });
                }
                GameMessage::GameStateUpdate {
                    turn, valid_move, ..
                } => {
                    info!("[GAME] State update: turn={}, valid={}", turn, valid_move);
                }
                GameMessage::GameEnd { winner, reason } => {
                    info!("[GAME] Game ended: winner={:?}, reason={}", winner, reason);
                }
                GameMessage::ChatMessage {
                    sender, content, ..
                } => {
                    info!("[CHAT] {}: {}", sender, content);
                    // Legacy/Lobby Chat
                    lobby_ui.chat_messages.push((sender, content));
                }
                GameMessage::CrdtOperation(op) => {
                    info!("[CHAT] Recv CRDT Op: {:?}", op.op_type);
                    chat_state.state.apply(op);
                }
                _ => {}
            }
        }
    }
}
