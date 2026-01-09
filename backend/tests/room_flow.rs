use backend::game;
use bevy::prelude::*;
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use shared::protocol::{Channel1, GameMessage, LobbyMessage, ProtocolPlugin};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

#[derive(Component, Debug, Default)]
struct TestClientState {
    step: usize,
    is_host: bool,
    is_white: bool,
}

#[test]
fn test_networked_moves() {
    // 1. Start Server
    std::thread::spawn(|| {
        game::run_game_server();
    });
    std::thread::sleep(Duration::from_secs(2));

    // 2. Setup Client App
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // app.add_plugins(bevy::log::LogPlugin::default()); // Disable logs to see stdout clearly, or keep for debugging
    app.add_plugins(ClientPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / 60.0),
    });
    app.add_plugins(ProtocolPlugin);

    app.add_systems(Startup, setup_clients);
    app.add_systems(Update, client_logic);

    // Run loop
    let start = std::time::Instant::now();

    // We run manually to check success condition
    while start.elapsed() < Duration::from_secs(10) {
        app.update();
        std::thread::sleep(Duration::from_millis(16));
    }
}

fn setup_clients(mut commands: Commands) {
    for i in 0..2 {
        let client_id = 1000 + i;
        spawn_client(&mut commands, client_id);
    }
}

fn spawn_client(commands: &mut Commands, client_id: u64) {
    let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 5000);
    // Server uses [0; 32] key. Matches backend/src/game.rs PRIVATE_KEY
    let key_bytes = [0u8; 32];
    let auth = Authentication::Manual {
        server_addr,
        client_id,
        private_key: Key::from(key_bytes),
        protocol_id: 0,
    };

    // Match client.rs config: infinite token expiry
    let netcode_config = NetcodeConfig {
        client_timeout_secs: 10,
        token_expire_secs: -1,
        ..default()
    };

    let client_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0); // System picks port

    let client_entity = commands.spawn(Client::default()).id();
    commands.entity(client_entity).insert((
        LocalAddr(client_addr),
        PeerAddr(server_addr), // Added PeerAddr as in client.rs
        Link::new(None),       // client.rs uses None
        ReplicationReceiver::default(),
        PredictionManager::default(),
        NetcodeClient::new(auth, netcode_config).expect("Failed to create client"),
        UdpIo::default(),
        MessageSender::<LobbyMessage>::default(),
        MessageReceiver::<LobbyMessage>::default(),
        MessageSender::<GameMessage>::default(),
        MessageReceiver::<GameMessage>::default(),
        Name::new(format!("Client_{}", client_id)),
        TestClientState::default(),
    ));

    commands.trigger(Connect {
        entity: client_entity,
    });
}

#[allow(clippy::type_complexity)]
fn client_logic(
    mut q: Query<
        (
            Entity,
            &Name,
            &mut TestClientState,
            &mut MessageSender<LobbyMessage>,
            &mut MessageReceiver<LobbyMessage>,
            &mut MessageSender<GameMessage>,
            &mut MessageReceiver<GameMessage>,
        ),
        With<Client>,
    >,
    connected: Query<Entity, (With<Client>, With<Connected>)>,
) {
    for (
        entity,
        name,
        mut state,
        mut lobby_sender,
        mut lobby_recv,
        mut game_sender,
        mut game_recv,
    ) in q.iter_mut()
    {
        // Step 0: Wait config
        if state.step == 0 {
            if connected.get(entity).is_ok() {
                println!("[{}] Connected. Sending JoinRoom(TEST_ROOM)", name);
                let _ = lobby_sender.send::<Channel1>(LobbyMessage::JoinRoom {
                    code: "TEST_ROOM".to_string(),
                });
                state.step = 1;
            }
        }

        // Step 1: Wait Joined, Send Ready
        if state.step == 1 {
            for msg in lobby_recv.receive() {
                if let LobbyMessage::JoinedRoom { is_host, .. } = msg {
                    println!("[{}] Joined Room (Host={})", name, is_host);
                    state.is_host = is_host;
                    let _ = lobby_sender.send::<Channel1>(LobbyMessage::SetReady { ready: true });
                    state.step = 2;
                }
            }
        }

        // Step 2: Wait Opponent Ready & Start logic
        if state.step == 2 {
            // Consume messages
            for msg in lobby_recv.receive() {
                match msg {
                    LobbyMessage::PlayerReady { .. } => {
                        println!("[{}] Saw PlayerReady", name);
                    }
                    LobbyMessage::GameStarting { your_color } => {
                        println!("[{}] Game Starting! Color White={}", name, your_color);
                        state.is_white = your_color;
                        state.step = 3;
                    }
                    _ => {}
                }
            }

            if state.is_host && state.step == 2 {
                let _ = lobby_sender.send::<Channel1>(LobbyMessage::StartGame);
            }
        }

        // Step 3: Game Started. White moves.
        if state.step == 3 {
            if state.is_white {
                println!("[{}] Sending Move 0,0 -> 1,1", name);
                let _ = game_sender.send::<Channel1>(GameMessage::SubmitMove {
                    from: (0, 0),
                    to: (1, 1),
                });
                state.step = 4; // Wait for echo
            } else {
                state.step = 4; // Wait for opponent move
            }
        }

        // Step 4: Wait for MoveMade
        if state.step == 4 {
            for msg in game_recv.receive() {
                if let GameMessage::MoveMade { from, to } = msg {
                    println!(
                        "[{}] VALIDATION SUCCESS: Received Move {:?}->{:?}",
                        name, from, to
                    );
                    state.step = 5; // Done
                }
            }
        }
    }
}
