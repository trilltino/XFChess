# Core Module

## Purpose

The Core module provides foundational infrastructure for the XFChess application, managing game state transitions, window configuration, settings persistence, and error handling. It serves as the backbone that orchestrates the application's lifecycle.

## Impact on Game

This module is essential for:
- **State Management**: Controls flow between menu, gameplay, and pause states
- **Settings Persistence**: Saves and restores user preferences (graphics, audio, controls)
- **Window Management**: Handles window configuration, resizing, and display modes
- **Error Recovery**: Centralized error handling prevents crashes and provides graceful degradation
- **Application Lifecycle**: Manages startup, shutdown, and state transitions

## Architecture/Key Components

### State Management

| Component | Purpose |
|-----------|---------|
| [`AppState`](states.rs:1) | Enum defining all application states: `Splash`, `MainMenu`, `Game`, `Pause`, `PieceViewer` |
| [`GameState`](states.rs:1) | In-game state machine: `None`, `Setup`, `Playing`, `GameOver` |
| [`state_lifecycle.rs`](state_lifecycle.rs) | Handles state entry/exit transitions and cleanup |

### Core Systems

| Module | Function |
|--------|----------|
| [`plugin.rs`](plugin.rs) | [`CorePlugin`](plugin.rs) - Registers all core systems and resources |
| [`window_config.rs`](window_config.rs) | Window settings, resolution, fullscreen mode |
| [`settings_persistence.rs`](settings_persistence.rs) | Save/load user preferences to disk |
| [`error_handling.rs`](error_handling.rs) | Global error handlers and recovery mechanisms |
| [`resources.rs`](resources.rs) | Shared game resources and configuration |

### State Transitions

```
[Splash] → [MainMenu] → [Game] ↔ [Pause]
              ↓              ↓
        [PieceViewer]   [GameOver]
```

## Usage

### Adding States

```rust
use crate::core::states::AppState;

fn main() {
    App::new()
        .add_state::<AppState>()
        .add_systems(Update, my_system.run_if(in_state(AppState::Game)))
        .run();
}
```

### State Transition

```rust
fn start_game(
    mut next_state: ResMut<NextState<AppState>>,
) {
    next_state.set(AppState::Game);
}
```

### Settings Persistence

```rust
fn save_settings(
    settings: Res<GameSettings>,
) {
    // Automatically persisted to disk
    // Loaded on next startup
}
```

## Dependencies

- [`bevy`](https://docs.rs/bevy) - Core game engine
- `serde` - Settings serialization
- `dirs` - Platform-appropriate config directories

## Related Modules

- [`states`](../states/README.md) - State-specific implementations
- [`game`](../game/README.md) - Core gameplay systems
- [`ui`](../ui/README.md) - UI that responds to state changes
