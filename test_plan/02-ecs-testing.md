# Bevy ECS Testing Guide

This guide covers testing Bevy applications using patterns from `reference/bevy/tests/`.

## Core Concepts

### Headless Testing with MinimalPlugins

Never use `DefaultPlugins` in tests - it requires a window and GPU:

```rust
use bevy::prelude::*;

fn create_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app
}

#[test]
fn test_basic_app() {
    let mut app = create_test_app();
    app.update();
    // App ran one frame successfully
}
```

### Testing State Transitions

From `tests/core_tests.rs`:

```rust
use bevy::prelude::*;
use xfchess::core::GameState;

#[test]
fn test_state_transition_to_ingame() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_state::<GameState>();
    
    // Trigger transition
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    
    app.update();
    
    // Verify state changed
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::InGame);
}
```

### Testing Systems with Run Conditions

```rust
#[derive(Resource, Default)]
struct ExecutionCounter(u32);

fn counting_system(mut counter: ResMut<ExecutionCounter>) {
    counter.0 += 1;
}

#[test]
fn test_system_runs_in_correct_state() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_state::<GameState>();
    app.init_resource::<ExecutionCounter>();
    
    app.add_systems(
        Update,
        counting_system.run_if(in_state(GameState::InGame))
    );
    
    // In MainMenu - system should NOT run
    app.update();
    assert_eq!(app.world().resource::<ExecutionCounter>().0, 0);
    
    // Transition to InGame
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    app.update();
    
    // System should have run once
    assert_eq!(app.world().resource::<ExecutionCounter>().0, 1);
}
```

### Mocking User Input

From `reference/bevy/tests/how_to_test_apps.rs`:

```rust
#[test]
fn test_keyboard_input_handling() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    
    // Insert input resource manually
    app.insert_resource(ButtonInput::<KeyCode>::default());
    
    // Simulate key press
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    
    // Add system that responds to input
    app.add_systems(Update, handle_space_key);
    
    app.update();
    
    // Clear input for next frame
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
}
```

### Testing Entity Spawning

```rust
#[derive(Component)]
struct Player { health: u32 }

fn spawn_player(mut commands: Commands) {
    commands.spawn(Player { health: 100 });
}

#[test]
fn test_player_spawns() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_systems(Startup, spawn_player);
    
    app.update(); // Run startup systems
    
    // Query for player
    let players: Vec<&Player> = app
        .world_mut()
        .query::<&Player>()
        .iter(app.world())
        .collect();
    
    assert_eq!(players.len(), 1);
    assert_eq!(players[0].health, 100);
}
```

### Testing Messages (Events)

From `reference/bevy/tests/how_to_test_systems.rs`:

```rust
#[derive(Message)]
struct DamageEvent { amount: u32 }

fn apply_damage(
    mut events: MessageReader<DamageEvent>,
    mut query: Query<&mut Health>,
) {
    for event in events.read() {
        for mut health in &mut query {
            health.current -= event.amount;
        }
    }
}

#[test]
fn test_damage_event_reduces_health() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<DamageEvent>();
    app.add_systems(Update, apply_damage);
    
    // Spawn entity with health
    let entity = app.world_mut().spawn(Health { current: 100 }).id();
    
    // Send damage event
    app.world_mut()
        .resource_mut::<Messages<DamageEvent>>()
        .write(DamageEvent { amount: 25 });
    
    app.update();
    
    // Verify health reduced
    let health = app.world().get::<Health>(entity).unwrap();
    assert_eq!(health.current, 75);
}
```

## XFChess-Specific Tests

### Testing Game Logic Systems

```rust
// tests/game_logic_tests.rs
use bevy::prelude::*;
use xfchess::game::{CurrentTurn, PieceColor};
use xfchess::rendering::pieces::Piece;

#[test]
fn test_turn_alternates_after_move() {
    let mut app = create_test_app();
    app.init_resource::<CurrentTurn>();
    
    // Initial turn is white
    assert_eq!(
        app.world().resource::<CurrentTurn>().color,
        PieceColor::White
    );
    
    // Simulate move completion
    app.world_mut().resource_mut::<CurrentTurn>().color = PieceColor::Black;
    
    assert_eq!(
        app.world().resource::<CurrentTurn>().color,
        PieceColor::Black
    );
}
```

### Testing Selection Resource

```rust
use xfchess::game::resources::Selection;

#[test]
fn test_selection_clear() {
    let mut selection = Selection::default();
    selection.selected_entity = Some(Entity::PLACEHOLDER);
    selection.possible_moves = vec![(0, 0), (1, 1)];
    
    selection.clear();
    
    assert!(selection.selected_entity.is_none());
    assert!(selection.possible_moves.is_empty());
}
```

## Test Utilities

Create a shared test utilities module:

```rust
// tests/common/mod.rs
use bevy::prelude::*;
use xfchess::core::GameState;

pub fn create_game_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_state::<GameState>();
    app.insert_resource(ButtonInput::<KeyCode>::default());
    app
}

pub fn set_state(app: &mut App, state: GameState) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(state);
    app.update();
}
```
