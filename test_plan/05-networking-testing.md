# Networking and Multiplayer Testing Guide

This guide covers testing Lightyear networking using patterns from `reference/lightyear/lightyear_tests/`.

## Core Concept: ClientServerStepper

Lightyear provides a deterministic testing harness that simulates client-server communication:

```rust
use lightyear_tests::stepper::{ClientServerStepper, StepperConfig};

#[test]
fn test_client_connects() {
    let mut stepper = ClientServerStepper::from_config(StepperConfig::single());
    
    // Client should be connected after init
    assert!(stepper.client(0).contains::<Connected>());
}
```

## Test Infrastructure

### Stepper Configuration

```rust
// Single client
let config = StepperConfig::single();

// Multiple clients
let config = StepperConfig::with_netcode_clients(2);

// Host server (client + server in same app)
let config = StepperConfig::host_server();
```

### Frame and Tick Stepping

```rust
#[test]
fn test_message_delivery() {
    let mut stepper = ClientServerStepper::from_config(StepperConfig::single());
    
    // Send message from client
    stepper.client_mut(0)
        .get_mut::<MessageSender<GameMessage>>()
        .unwrap()
        .send::<Channel1>(GameMessage::SubmitMove { from: (0,0), to: (1,1) });
    
    // Advance simulation
    stepper.frame_step(5);
    
    // Check server received message
    // ...
}
```

## XFChess Multiplayer Tests

### Room Flow Test (Existing)

From `backend/tests/room_flow.rs`:

```rust
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
    app.add_plugins(ClientPlugins { tick_duration: Duration::from_secs_f64(1.0/60.0) });
    app.add_plugins(ProtocolPlugin);
    
    // 3. Spawn two clients
    app.add_systems(Startup, setup_clients);
    app.add_systems(Update, client_logic);
    
    // 4. Run simulation
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        app.update();
        std::thread::sleep(Duration::from_millis(16));
    }
}
```

### Protocol Message Testing

```rust
// crates/shared/src/protocol.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lobby_message_serialization() {
        let msg = LobbyMessage::JoinRoom { code: "ABCD".to_string() };
        let bytes = bincode::serialize(&msg).unwrap();
        let decoded: LobbyMessage = bincode::deserialize(&bytes).unwrap();
        
        match decoded {
            LobbyMessage::JoinRoom { code } => assert_eq!(code, "ABCD"),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_game_message_move() {
        let msg = GameMessage::MoveMade { from: (4, 1), to: (4, 3) };
        let bytes = bincode::serialize(&msg).unwrap();
        let decoded: GameMessage = bincode::deserialize(&bytes).unwrap();
        
        assert!(matches!(decoded, GameMessage::MoveMade { from: (4, 1), to: (4, 3) }));
    }
}
```

### Connection State Testing

```rust
#[test]
fn test_client_reconnection() {
    let mut stepper = ClientServerStepper::from_config(StepperConfig::single());
    
    // Disconnect client
    stepper.disconnect_client();
    stepper.frame_step(5);
    
    // Reconnect
    stepper.new_client(ClientType::Netcode, None);
    stepper.wait_for_connection();
    
    assert!(stepper.client(0).contains::<Connected>());
}
```

## Testing Network Events

### Testing NetworkMoveEvent

```rust
// tests/networking_tests.rs
use bevy::prelude::*;
use xfchess::game::events::NetworkMoveEvent;

#[test]
fn test_network_move_event_handling() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<NetworkMoveEvent>();
    
    // Spawn a mock piece
    let piece_entity = app.world_mut().spawn(Piece {
        x: 4, y: 1,
        color: PieceColor::White,
        piece_type: PieceType::Pawn,
    }).id();
    
    // Send network move event
    app.world_mut()
        .resource_mut::<Messages<NetworkMoveEvent>>()
        .write(NetworkMoveEvent { from: (4, 1), to: (4, 3) });
    
    app.add_systems(Update, handle_network_moves);
    app.update();
    
    // Verify piece moved
    let piece = app.world().get::<Piece>(piece_entity).unwrap();
    assert_eq!((piece.x, piece.y), (4, 3));
}
```

## Mock Network Layer

For unit tests without real networking:

```rust
// tests/common/mock_network.rs
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct MockNetworkReceiver {
    pub messages: Vec<GameMessage>,
}

#[derive(Resource, Default)]
pub struct MockNetworkSender {
    pub sent: Vec<GameMessage>,
}

pub fn mock_receive_system(
    mut receiver: ResMut<MockNetworkReceiver>,
    mut events: MessageWriter<NetworkMoveEvent>,
) {
    for msg in receiver.messages.drain(..) {
        if let GameMessage::MoveMade { from, to } = msg {
            events.write(NetworkMoveEvent { from, to });
        }
    }
}
```

## Running Network Tests

```bash
# Run backend/network tests
cargo test -p backend

# Run with real server (slow)
cargo test -p backend --features real-server -- --ignored

# Run shared protocol tests
cargo test -p shared
```
