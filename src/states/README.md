# States Module

## Purpose

The States module implements Bevy state-specific plugins for each game screen in XFChess. Each state encapsulates its own systems, UI, and behavior, providing clean separation between menu, gameplay, and other application modes.

## Impact on Game

This module organizes:
- **Main Menu**: Primary navigation with game mode selection
- **Game State**: Active gameplay with board and piece interaction
- **Pause Menu**: In-game menu for settings and exit options
- **Game Over**: Match end screen with results and rematch options
- **Piece Viewer**: 3D model inspection mode
- **Multiplayer Menu**: Lobby and connection interface

## Architecture/Key Components

### State Plugins

| Plugin | File | Purpose |
|--------|------|---------|
| [`MainMenuPlugin`](main_menu.rs) | `main_menu.rs` | Main menu UI and navigation |
| [`MainMenuShowcase`](main_menu_showcase.rs) | `main_menu_showcase.rs` | Visual showcase in main menu |
| [`GameOverPlugin`](game_over.rs) | `game_over.rs` | End-game screen and results |
| [`PausePlugin`](pause.rs) | `pause.rs` | Pause menu overlay |
| [`PieceViewerPlugin`](piece_viewer.rs) | `piece_viewer.rs` | 3D piece inspection mode |
| [`MultiplayerMenuPlugin`](multiplayer_menu.rs) | `multiplayer_menu.rs` | Multiplayer lobby UI |

### State Lifecycle

Each state plugin follows this pattern:

```rust
// On Enter: Setup systems run once
app.add_systems(OnEnter(AppState::Game), setup_game);

// On Update: Systems run every frame
app.add_systems(Update, game_logic.run_if(in_state(AppState::Game)));

// On Exit: Cleanup systems run once
app.add_systems(OnExit(AppState::Game), cleanup_game);
```

### State Transitions

```
                    ┌─────────────────┐
                    │   Main Menu     │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐   ┌─────────────────┐   ┌──────────────┐
│ Piece Viewer  │   │  Multiplayer    │   │ Singleplayer │
│   (Showcase)  │   │     Menu        │   │    Game      │
└───────────────┘   └────────┬────────┘   └──────┬───────┘
                             │                    │
                             └────────┬───────────┘
                                      ▼
                            ┌─────────────────┐
                            │  Active Game    │◄──────┐
                            └────────┬────────┘       │
                                     │                │
                              ┌──────┴──────┐         │
                              ▼             ▼         │
                      ┌──────────┐   ┌──────────┐     │
                      │   Pause  │   │ Game Over│     │
                      └────┬─────┘   └────┬─────┘     │
                           │              │           │
                           └──────┬───────┘           │
                                  └───────────────────┘
```

## Usage

### Implementing a State Plugin

```rust
pub struct MyStatePlugin;

impl Plugin for MyStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MyState), setup)
           .add_systems(Update, update.run_if(in_state(AppState::MyState)))
           .add_systems(OnExit(AppState::MyState), cleanup);
    }
}

fn setup(mut commands: Commands) {
    // Spawn UI, load resources
}

fn update() {
    // State-specific logic
}

fn cleanup(mut commands: Commands, query: Query<Entity, With<MyStateUI>>) {
    // Despawn entities, clean up
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}
```

### State Transitions

```rust
fn start_game(mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::Game);
}

fn return_to_menu(mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::MainMenu);
}

fn pause_game(mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::Pause);
}

fn resume_game(mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::Game);
}
```

## Dependencies

- [`bevy`](https://docs.rs/bevy) - State management systems
- [`core`](../core/README.md) - Core state definitions
- [`ui`](../ui/README.md) - UI components for states
- [`egui`](https://docs.rs/egui) - Immediate mode UI

## Related Modules

- [`core`](../core/README.md) - Defines AppState enum and lifecycle
- [`ui`](../ui/README.md) - Shared UI components
- [`game`](../game/README.md) - Game logic during Game state
- [`multiplayer`](../multiplayer/README.md) - Multiplayer during Game state
