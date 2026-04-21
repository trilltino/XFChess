# XFChess Source Code

This directory contains the main source code for the XFChess game client, built on the Bevy game engine.

## Architecture Overview

XFChess is a 3D chess game that integrates with Solana blockchain for game history and tournament management. The codebase follows an ECS (Entity-Component-System) architecture through Bevy, with clear separation between game logic, rendering, networking, and UI layers.

## Directory Structure

- **core/** - Core game framework, state management, and shared types
  - Contains the fundamental game state machine and configuration structures
  - Defines game modes (SinglePlayer, MultiplayerLocal, Tournament)
  - Manages menu states and transitions

- **game/** - Game logic, systems, and AI
  - **systems/** - Bevy systems for game logic (input, camera, movement, etc.)
  - **ai/** - Stockfish AI integration with configurable difficulty
  - **sync.rs** - Network synchronization for multiplayer
  - Components define chess pieces, board state, and game metadata

- **multiplayer/** - Multiplayer networking and protocols
  - **network/** - P2P networking layer
  - **solana/** - Solana blockchain integration for on-chain game recording
  - Protocol definitions for network communication

- **ui/** - User interface components and menus
  - **menus/** - Main menu, mode selection, tournament lobby
  - **system_params.rs** - UI state management
  - egui-based UI system integrated with Bevy

- **input/** - Input handling and event processing
  - Mouse and keyboard input systems
  - Piece selection and movement logic
  - Camera control input

- **rendering/** - 3D rendering and graphics
  - **pieces/** - 3D piece models and materials
  - Board rendering and visual effects
  - Camera systems and view modes

- **presentation/** - Presentation layer for UI
  - UI component styling and theming
  - Layout management

- **singleplayer/** - Singleplayer game mode
  - Local gameplay systems
  - AI opponent management

- **solana/** - Solana blockchain integration
  - Wallet connection and transaction signing
  - Tournament registration and management
  - Blinks action integration

- **states/** - Game states and state transitions
  - GameState enum (MainMenu, InGame, Tournament, etc.)
  - State transition logic
  - Menu state management

- **assets/** - Asset management and loading
  - 3D models, textures, sounds
  - Asset loading pipeline

- **cli/** - Command-line interface tools
  - Tournament administration
  - Debug utilities

- **bin/** - Binary utilities and debug tools
  - Debugger, PDA utilities, tournament testing

## Key Files

- `lib.rs` - Main library module exports and public API
- `main.rs` - Application entry point and Bevy app initialization

## Technology Stack

- **Bevy** - ECS-based game engine for Rust
- **egui** - Immediate mode GUI library for menus
- **Stockfish** - Chess engine for AI opponents
- **Solana SDK** - Blockchain integration
- **Anchor** - Solana smart contract framework

## Game Flow

1. **Initialization**: Bevy app starts with plugins registered
2. **Main Menu**: Player selects game mode (Singleplayer, Multiplayer, Tournament)
3. **Game Setup**: Board is initialized with pieces
4. **Gameplay**: Players take turns making moves
5. **Game End**: Winner is determined and game is recorded (if on-chain)

## Example: Game Configuration

```rust
use xfchess::GameConfig;

/// Configuration structure for the XFChess game client
#[derive(Clone, Debug)]
pub struct GameConfig {
    /// Window width in pixels
    pub window_width: u32,
    /// Window height in pixels  
    pub window_height: u32,
    /// Enable vertical sync
    pub vsync: bool,
    /// Enable Solana features
    pub enable_solana: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            window_width: 1920,
            window_height: 1080,
            vsync: true,
            enable_solana: false,
        }
    }
}

fn main() {
    let config = GameConfig {
        window_width: 1920,
        window_height: 1080,
        vsync: true,
        ..Default::default()
    };
}
```

## Example: Adding a Custom Bevy System

Bevy uses an ECS (Entity-Component-System) architecture. Systems are functions that run on queries of entities with specific components.

```rust
use bevy::prelude::*;

/// Component marker for entities that need custom processing
#[derive(Component)]
struct MyComponent;

/// Custom system that processes entities with MyComponent and Transform
/// 
/// # Arguments
/// * `query` - Query for entities that have both MyComponent and Transform
/// 
/// # System Schedule
/// This system runs during the Update schedule, which runs once per frame
fn my_custom_system(
    query: Query<&Transform, With<MyComponent>>,
) {
    for transform in query.iter() {
        // Process each entity's transform
        // Example: log position, apply modifications, etc.
    }
}

/// Plugin that registers the custom system with the Bevy app
/// 
/// Plugins are the standard way to extend Bevy applications
impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        // Add the system to the Update schedule
        app.add_systems(Update, my_custom_system);
    }
}
```

## Example: Game State Management

XFChess uses a state machine to manage game flow:

```rust
/// Represents the current state of the game
#[derive(Clone, Copy, Debug, PartialEq, Eq, States)]
pub enum GameState {
    /// Player is in the main menu
    MainMenu,
    /// Player is selecting a game mode
    ModeSelect,
    /// Game is actively being played
    InGame,
    /// Player is in a tournament
    Tournament,
    /// Game is paused
    Paused,
}

/// Transitions the game from one state to another
fn transition_to_in_game(
    mut next_state: ResMut<NextState<GameState>>,
) {
    next_state.set(GameState::InGame);
}
```
