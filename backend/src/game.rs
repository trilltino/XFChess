use bevy::log::LogPlugin;
use bevy::prelude::*;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use shared::protocol::{Channel1, GameMessage, LobbyMessage, ProtocolPlugin};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use lightyear::netcode::server_plugin::NetcodeConfig;

pub const PROTOCOL_ID: u64 = 0;
pub const PRIVATE_KEY: [u8; 32] = [0; 32];

/// Marker component for our main server entity
#[derive(Component)]
struct GameServer;

/// A game room with two players
#[derive(Debug, Clone)]
struct GameRoom {
    code: String,
    host_entity: Entity,
    host_peer_id: PeerId,
    host_ready: bool,
    guest_entity: Option<Entity>,
    guest_peer_id: Option<PeerId>,
    guest_ready: bool,
    in_game: bool,
}

/// Resource to track all active rooms
#[derive(Resource, Default)]
struct GameRooms {
    rooms: HashMap<String, GameRoom>,
    entity_to_room: HashMap<Entity, String>,
}

impl GameRooms {
    fn generate_code() -> String {
        use rand::Rng;
        let mut rng = rand::rng();
        (0..6)
            .map(|_| rng.random_range(b'A'..=b'Z') as char)
            .collect()
    }
}

pub fn run_game_server() {
    println!("[BACKEND] Starting Lightyear Game Server on port 5000...");

    let mut app = App::new();

    app.add_plugins(MinimalPlugins);
    app.add_plugins(LogPlugin::default());

    app.add_plugins(ServerPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / 60.0),
    });

    app.add_plugins(ProtocolPlugin);
    app.init_resource::<GameRooms>();

    app.add_systems(Startup, setup_server);
    app.add_systems(
        Update,
        (
            track_connections,
            handle_lobby_messages,
            handle_game_messages,
        ),
    );

    println!("[BACKEND] Running Lightyear server...");
    app.run();
}

fn setup_server(mut commands: Commands) {
    let server_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 5000);

    let netcode_config = NetcodeConfig::default()
        .with_protocol_id(PROTOCOL_ID)
        .with_key(PRIVATE_KEY);

    let server_entity = commands
        .spawn((
            GameServer,
            NetcodeServer::new(netcode_config),
            ServerUdpIo::default(),
            LocalAddr(server_addr),
        ))
        .id();

    // Trigger Start to begin listening
    commands.trigger(Start {
        entity: server_entity,
    });

    println!("[BACKEND] Server started on port 5000");
}

/// Track client connections/disconnections
fn track_connections(
    mut commands: Commands,
    connected: Query<(Entity, &RemoteId), Added<Connected>>,
    disconnected: Query<(Entity, &RemoteId), Added<Disconnected>>,
    mut rooms: ResMut<GameRooms>,
    mut senders: Query<&mut MessageSender<LobbyMessage>>,
) {
    for (entity, remote_id) in connected.iter() {
        println!(
            "[BACKEND] Client connected: {:?} (Entity: {:?})",
            remote_id.0, entity
        );
        commands.entity(entity).insert((
            MessageReceiver::<LobbyMessage>::default(),
            MessageSender::<LobbyMessage>::default(),
            MessageReceiver::<GameMessage>::default(),
            MessageSender::<GameMessage>::default(),
        ));
    }

    for (entity, remote_id) in disconnected.iter() {
        println!("[BACKEND] Client disconnected: {:?}", remote_id.0);

        // Remove from room and notify other player
        if let Some(room_code) = rooms.entity_to_room.remove(&entity) {
            if let Some(room) = rooms.rooms.get_mut(&room_code) {
                if room.host_entity == entity {
                    // Host left - notify guest and remove room
                    if let Some(guest_entity) = room.guest_entity {
                        if let Ok(mut sender) = senders.get_mut(guest_entity) {
                            let _ = sender.send::<Channel1>(LobbyMessage::Error {
                                message: "Host disconnected".to_string(),
                            });
                        }
                        rooms.entity_to_room.remove(&guest_entity);
                    }
                    rooms.rooms.remove(&room_code);
                } else if room.guest_entity == Some(entity) {
                    // Guest left - notify host
                    room.guest_entity = None;
                    room.guest_peer_id = None;
                    room.guest_ready = false;
                    if let Ok(mut sender) = senders.get_mut(room.host_entity) {
                        let _ = sender.send::<Channel1>(LobbyMessage::PlayerLeft {
                            player_id: remote_id.0.to_bits(),
                        });
                    }
                }
            }
        }
    }
}

/// Handle lobby messages
fn handle_lobby_messages(
    mut query: Query<(Entity, &RemoteId, &mut MessageReceiver<LobbyMessage>)>,
    mut senders: Query<&mut MessageSender<LobbyMessage>>,
    game_senders: Query<&mut MessageSender<GameMessage>>,
    mut rooms: ResMut<GameRooms>,
) {
    // Collect actions to avoid borrow conflicts
    let mut actions: Vec<(Entity, PeerId, LobbyMessage)> = Vec::new();

    for (entity, remote_id, mut receiver) in query.iter_mut() {
        for msg in receiver.receive() {
            actions.push((entity, remote_id.0, msg.clone()));
        }
    }

    for (entity, peer_id, msg) in actions {
        match msg {
            LobbyMessage::CreateRoom => {
                let code = GameRooms::generate_code();
                println!("[BACKEND] Creating room {} for {:?}", code, peer_id);

                rooms.rooms.insert(
                    code.clone(),
                    GameRoom {
                        code: code.clone(),
                        host_entity: entity,
                        host_peer_id: peer_id,
                        host_ready: false,
                        guest_entity: None,
                        guest_peer_id: None,
                        guest_ready: false,
                        in_game: false,
                    },
                );
                rooms.entity_to_room.insert(entity, code.clone());

                if let Ok(mut sender) = senders.get_mut(entity) {
                    let _ =
                        sender.send::<Channel1>(LobbyMessage::RoomCreated { code: code.clone() });
                    let _ = sender.send::<Channel1>(LobbyMessage::JoinedRoom {
                        code,
                        is_host: true,
                    });
                }
            }

            LobbyMessage::JoinRoom { code } => {
                println!("[BACKEND] Player {:?} joining room {}", peer_id, code);

                // First check room state without holding mutable borrow
                let room_state = rooms
                    .rooms
                    .get(&code)
                    .map(|room| (room.guest_entity.is_some(), room.in_game, room.host_entity));

                match room_state {
                    None => {
                        println!(
                            "[BACKEND] Room {} not found, creating new room for {:?}",
                            code, peer_id
                        );
                        rooms.rooms.insert(
                            code.clone(),
                            GameRoom {
                                code: code.clone(),
                                host_entity: entity,
                                host_peer_id: peer_id,
                                host_ready: false,
                                guest_entity: None,
                                guest_peer_id: None,
                                guest_ready: false,
                                in_game: false,
                            },
                        );
                        rooms.entity_to_room.insert(entity, code.clone());

                        if let Ok(mut sender) = senders.get_mut(entity) {
                            let _ = sender
                                .send::<Channel1>(LobbyMessage::RoomCreated { code: code.clone() });
                            let _ = sender.send::<Channel1>(LobbyMessage::JoinedRoom {
                                code,
                                is_host: true,
                            });
                        }
                    }
                    Some((true, _, _)) => {
                        if let Ok(mut sender) = senders.get_mut(entity) {
                            let _ = sender.send::<Channel1>(LobbyMessage::Error {
                                message: "Room is full".to_string(),
                            });
                        }
                    }
                    Some((_, true, _)) => {
                        if let Ok(mut sender) = senders.get_mut(entity) {
                            let _ = sender.send::<Channel1>(LobbyMessage::Error {
                                message: "Game already started".to_string(),
                            });
                        }
                    }
                    Some((false, false, host_entity)) => {
                        // Update room state
                        if let Some(room) = rooms.rooms.get_mut(&code) {
                            room.guest_entity = Some(entity);
                            room.guest_peer_id = Some(peer_id);
                        }
                        rooms.entity_to_room.insert(entity, code.clone());

                        // Notify joiner
                        if let Ok(mut sender) = senders.get_mut(entity) {
                            let _ = sender.send::<Channel1>(LobbyMessage::JoinedRoom {
                                code: code.clone(),
                                is_host: false,
                            });
                        }

                        // Notify host
                        if let Ok(mut sender) = senders.get_mut(host_entity) {
                            let _ = sender.send::<Channel1>(LobbyMessage::PlayerJoined {
                                player_id: peer_id.to_bits(),
                            });
                        }
                    }
                }
            }

            LobbyMessage::SetReady { ready } => {
                if let Some(room_code) = rooms.entity_to_room.get(&entity).cloned() {
                    if let Some(room) = rooms.rooms.get_mut(&room_code) {
                        let is_host = room.host_entity == entity;
                        if is_host {
                            room.host_ready = ready;
                        } else {
                            room.guest_ready = ready;
                        }

                        println!(
                            "[BACKEND] Player {:?} ready={} in room {}",
                            peer_id, ready, room_code
                        );

                        // Notify both players
                        for target in [Some(room.host_entity), room.guest_entity]
                            .into_iter()
                            .flatten()
                        {
                            if let Ok(mut sender) = senders.get_mut(target) {
                                let _ = sender.send::<Channel1>(LobbyMessage::PlayerReady {
                                    player_id: peer_id.to_bits(),
                                    ready,
                                });
                            }
                        }
                    }
                }
            }

            LobbyMessage::StartGame => {
                if let Some(room_code) = rooms.entity_to_room.get(&entity).cloned() {
                    if let Some(room) = rooms.rooms.get_mut(&room_code) {
                        // Only host can start, both must be ready, guest must be present
                        if room.host_entity == entity
                            && room.host_ready
                            && room.guest_ready
                            && room.guest_entity.is_some()
                        {
                            room.in_game = true;
                            println!("[BACKEND] Starting game in room {}", room_code);

                            // Notify host (White)
                            if let Ok(mut sender) = senders.get_mut(room.host_entity) {
                                let _ = sender.send::<Channel1>(LobbyMessage::GameStarting {
                                    your_color: true,
                                });
                            }

                            // Notify guest (Black)
                            if let Some(guest) = room.guest_entity {
                                if let Ok(mut sender) = senders.get_mut(guest) {
                                    let _ = sender.send::<Channel1>(LobbyMessage::GameStarting {
                                        your_color: false,
                                    });
                                }
                            }
                        } else {
                            if let Ok(mut sender) = senders.get_mut(entity) {
                                let _ = sender.send::<Channel1>(LobbyMessage::Error {
                                    message: "Cannot start: need both players ready".to_string(),
                                });
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }
}

/// Handle in-game messages and broadcast to opponent
fn handle_game_messages(
    mut query: Query<(Entity, &RemoteId, &mut MessageReceiver<GameMessage>)>,
    mut senders: Query<&mut MessageSender<GameMessage>>,
    rooms: Res<GameRooms>,
) {
    let mut broadcasts: Vec<(Entity, GameMessage)> = Vec::new();

    for (entity, remote_id, mut receiver) in query.iter_mut() {
        for msg in receiver.receive() {
            if let Some(room_code) = rooms.entity_to_room.get(&entity) {
                if let Some(room) = rooms.rooms.get(room_code) {
                    if !room.in_game {
                        continue;
                    }

                    // Find opponent
                    let opponent = if room.host_entity == entity {
                        room.guest_entity
                    } else {
                        Some(room.host_entity)
                    };

                    if let Some(opp_entity) = opponent {
                        match msg {
                            GameMessage::SubmitMove { from, to } => {
                                let broadcast_msg = GameMessage::MoveMade { from, to };
                                broadcasts.push((opp_entity, broadcast_msg.clone()));
                                broadcasts.push((entity, broadcast_msg));
                            }
                            _ => {
                                broadcasts.push((opp_entity, msg.clone()));
                            }
                        }
                        // Chat should be echoed?
                        // If I echo ChatMessage, the sender sees it twice if they added it locally?
                        // Ui/chat.rs adds it locally. So sender should NOT receive echo of ChatMessage?
                        // But current logic echoes EVERYTHING.
                        // I will fix echo logic: send to opponent only, unless specific type needs ack.
                        // But wait, `chat.rs` sends `CrdtOperation`. That needs to be echoed?
                        // CRDT usually needs all peers to see ops?
                        // If I change this now, I might break chat.
                        // I'll stick to: SubmitMove -> MoveMade (broadcast).
                        // Chat/CRDT -> broadcast to opponent. (And sender? old logic did)

                        // OLD LOGIC: broadcasts.push((entity, msg.clone())); // Echo back

                        // New Logic:
                        // If SubmitMove -> Broadcast MoveMade to ALL.
                        // If others -> Broadcast to Opponent. (Sender logic for chat handled locally?)

                        // Let's keep it simple:
                        // MoveMade -> All.
                        // Others (inc CRDT) -> All (CRDT needs causal consistency usually, but if local applied, echo might duplicate?
                        //  chat.rs `send_message_local` applies locally.
                        //  If server echoes `CrdtOperation`, local client applies it AGAin.
                        //  `ChatState` CRDT likely handles idempotency OR we should NOT echo.

                        // Let's assume CRDT *should* be echoed for consistency or filtered by sender ID.
                        // But `SubmitMove` definitely needs transformation.
                    }
                }
            }
        }
    }

    for (target, msg) in broadcasts {
        if let Ok(mut sender) = senders.get_mut(target) {
            let _ = sender.send::<Channel1>(msg);
        }
    }
}
