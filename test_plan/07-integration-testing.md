# Integration Testing Guide

Integration tests verify that multiple modules work together correctly.

## Test Location

```
tests/
├── core_tests.rs        # State management (existing)
├── game_flow_tests.rs   # Full game scenarios
├── ai_integration.rs    # AI + board integration
└── common/
    └── mod.rs           # Shared test utilities
```

## Full Game Flow Tests

### Complete Game Scenario

```rust
// tests/game_flow_tests.rs
use bevy::prelude::*;
use xfchess::core::GameState;
use xfchess::game::{CurrentTurn, ChessEngine};
use xfchess::rendering::pieces::{Piece, PieceColor, PieceType};

mod common;
use common::*;

#[test]
fn test_complete_game_flow() {
    let mut app = create_full_test_app();
    
    // 1. Start in MainMenu
    assert_state(&app, GameState::MainMenu);
    
    // 2. Transition to InGame
    transition_to(&mut app, GameState::InGame);
    
    // 3. Verify pieces spawned
    let piece_count = count_pieces(&app);
    assert_eq!(piece_count, 32, "Should have 32 pieces");
    
    // 4. Simulate a move
    simulate_move(&mut app, (4, 1), (4, 3)); // e2-e4
    
    // 5. Verify turn changed
    let turn = app.world().resource::<CurrentTurn>();
    assert_eq!(turn.color, PieceColor::Black);
}

// Helper functions
fn create_full_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_state::<GameState>();
    app.init_resource::<CurrentTurn>();
    app.init_resource::<ChessEngine>();
    // Add game systems...
    app
}

fn count_pieces(app: &App) -> usize {
    app.world()
        .query::<&Piece>()
        .iter(app.world())
        .len()
}
```

### AI Integration Test

```rust
// tests/ai_integration.rs
use chess_engine::{new_game, reply, do_move};

#[test]
fn test_ai_responds_to_player_move() {
    let mut game = new_game();
    
    // Player moves e2-e4
    do_move(&mut game, 12, 28, true);
    
    // AI responds
    game.secs_per_move = 0.5;
    let ai_move = reply(&mut game);
    
    // AI made a valid move
    assert!(ai_move.src >= 0 && ai_move.src < 64);
    assert!(ai_move.dst >= 0 && ai_move.dst < 64);
    assert_ne!(ai_move.src, ai_move.dst);
}

#[test]
fn test_full_game_until_checkmate() {
    let mut game = new_game();
    game.secs_per_move = 0.1;
    
    let mut moves = 0;
    while game.state == STATE_PLAYING && moves < 200 {
        let mv = reply(&mut game);
        do_move(&mut game, mv.src, mv.dst, true);
        moves += 1;
    }
    
    // Game should end eventually
    assert!(
        game.state == STATE_CHECKMATE || game.state == STATE_STALEMATE || moves >= 200,
        "Game should conclude or reach move limit"
    );
}
```

### Board-Engine Synchronization

```rust
#[test]
fn test_ecs_engine_sync() {
    let mut app = create_game_test_app();
    
    // Spawn pieces via ECS
    spawn_starting_pieces(&mut app);
    
    // Get engine
    let engine = app.world().resource::<ChessEngine>();
    
    // Sync ECS -> Engine
    sync_ecs_to_engine(&mut app);
    
    // Verify sync
    let engine = app.world().resource::<ChessEngine>();
    assert_eq!(engine.game.board[0], ROOK);  // a1
    assert_eq!(engine.game.board[4], KING);  // e1
}
```

## Cross-Crate Integration

### shared + backend

```rust
// backend/tests/protocol_integration.rs
use shared::protocol::{LobbyMessage, GameMessage};

#[tokio::test]
async fn test_message_round_trip() {
    let server = spawn_test_server().await;
    let client = connect_client(&server.addr).await;
    
    // Send lobby message
    client.send(LobbyMessage::JoinRoom { code: "TEST".to_string() }).await;
    
    // Receive response
    let response = client.recv().await;
    assert!(matches!(response, LobbyMessage::JoinedRoom { .. }));
}
```

### chess_engine + game

```rust
// tests/engine_game_integration.rs
use xfchess::game::systems::shared::execute_move;

#[test]
fn test_execute_move_updates_engine() {
    let mut app = create_game_test_app();
    spawn_starting_pieces(&mut app);
    
    // Get pawn entity at e2
    let pawn = find_piece_at(&app, 4, 1).unwrap();
    
    // Execute move
    app.world_mut().run_system_once(|
        mut commands: Commands,
        mut engine: ResMut<ChessEngine>,
        // ... other params
    | {
        execute_move(
            "test",
            &mut commands,
            pawn,
            piece,
            (4, 3),
            None,
            true,
            // ... other params
        );
    });
    
    app.update();
    
    // Verify engine state updated
    let engine = app.world().resource::<ChessEngine>();
    assert_eq!(engine.game.board[28], PAWN); // e4 has pawn
    assert_eq!(engine.game.board[12], 0);    // e2 is empty
}
```

## Test Utilities Module

```rust
// tests/common/mod.rs
use bevy::prelude::*;
use xfchess::core::GameState;

pub fn create_game_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_state::<GameState>();
    // Add resources...
    app
}

pub fn assert_state(app: &App, expected: GameState) {
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), expected);
}

pub fn transition_to(app: &mut App, state: GameState) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(state);
    app.update();
}

pub fn spawn_starting_pieces(app: &mut App) {
    // Spawn all 32 pieces...
}

pub fn find_piece_at(app: &App, x: u8, y: u8) -> Option<Entity> {
    app.world()
        .query::<(Entity, &Piece)>()
        .iter(app.world())
        .find(|(_, p)| p.x == x && p.y == y)
        .map(|(e, _)| e)
}
```

## Running Integration Tests

```bash
# Run all integration tests
cargo test --test '*'

# Run specific test file
cargo test --test game_flow_tests

# Run with backtrace
RUST_BACKTRACE=1 cargo test --test ai_integration
```
