//! XFChess - A 3D Chess Game built with Bevy 0.17.2
//!
//! # Architecture
//!
//! This application demonstrates modern Bevy 0.17 ECS patterns including:
//! - State management with `GameState` enum for menu/gameplay transitions
//! - Plugin-based architecture for modular game systems
//! - Observer pattern using `.observe()` for entity-specific events
//! - Reflection system for runtime inspection with bevy-inspector-egui
//! - Async AI computation using `AsyncComputeTaskPool`
//!
//! # Plugin Loading Order
//!
//! The order of plugin registration is critical for proper system scheduling:
//!
//! 1. **Core Bevy Plugins** - DefaultPlugins, EguiPlugin, WorldInspectorPlugin
//! 2. **State Initialization** - GameState (LaunchMenu → Multiplayer transitions)
//! 3. **Input Systems** - MeshPickingPlugin, PointerEventsPlugin (entity selection)
//! 4. **Rendering** - PiecePlugin, BoardPlugin, BoardUtils (3D visualization)
//! 5. **Game Logic** - GamePlugin (rules, AI, move validation)
//! 6. **UI** - Launchmenu (menu screens, in-game HUD)
//!
//! Systems within plugins use `in_state(GameState::Multiplayer)` run conditions
//! to execute only during active gameplay, not in menus.
//!
//! # Reference Materials
//!
//! - `reference/bevy/` - Bevy 0.17 source and examples for API patterns
//! - `reference/bevy-inspector-egui/` - Inspector integration examples
//! - `reference/bevy-3d-chess/` - Alternative chess implementation for comparison
//! - `reference/chess_engine/` - Modularized chess engine with alpha-beta pruning
//!
//! # Module Organization
//!
//! - `core` - Game state management and shared types
//! - `rendering` - 3D board, pieces, and visual utilities
//! - `ui` - EGUI interfaces (menu, in-game HUD, inspector toggle)
//! - `input` - Pointer events and piece selection via observers
//! - `game` - Chess rules, move validation, AI opponent, and game logic
//!
//! # Key Features
//!
//! - **Chess AI**: Full minimax engine with alpha-beta pruning (~1800-2000 ELO)
//! - **3D Graphics**: Mesh-based pieces with highlighting and smooth animations
//! - **Game Modes**: Human vs Human, Human vs AI (configurable difficulty)
//! - **Time Control**: Fischer increment timer with timeout detection
//! - **Move Validation**: Complete chess rules including check, checkmate, stalemate
//! - **Inspector UI**: F1 to toggle ECS entity/component inspector for debugging

use bevy::prelude::*;
use bevy::gltf::Gltf;
use bevy::input::common_conditions::input_toggle_active;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

// Module declarations
mod core;
mod rendering;
mod ui;
mod input;
mod game;

// Imports
use core::{GameState, LaunchMenu};
use rendering::*;
use ui::*;
use input::*;
use game::{GamePlugin, CameraController};

const WINDOW_WIDTH: u32 = 1366;
const WINDOW_HEIGHT: u32 = 768;

fn main() {
    let window = Window {
        resolution: (WINDOW_WIDTH, WINDOW_HEIGHT).into(),
        ..default()
    };
    let primary_window = Some(window);

    App::new()
        // Core plugins
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window,
            ..default()
        }))
        .add_plugins(EguiPlugin::default())

        // Inspector (toggle with F1 key) - TEMPORARILY DISABLED FOR DEBUGGING
        // .add_plugins(
        //     WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::F1)),
        // )

        // Game state
        .init_state::<GameState>()
        .add_computed_state::<LaunchMenu>()

        // Game systems
        .add_plugins(MeshPickingPlugin)
        .add_plugins(PiecePlugin)
        .add_plugins(GamePlugin)
        .add_plugins(BoardPlugin)
        .add_plugins(BoardUtils)
        .add_plugins(PointerEventsPlugin)
        .add_plugins(Launchmenu {
            state: GameState::LaunchMenu,
        })

        // Startup systems
        .add_systems(OnEnter(GameState::LaunchMenu), setup_launch_camera)
        .add_systems(OnEnter(GameState::Multiplayer), setup_game_camera)
        .add_systems(Startup, (setup, preload_chess_assets))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Lighting
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 100000.0,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
        Name::new("Main Light"),
    ));

    // Background
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("000000")
            .expect("hardcoded hex color '000000' is valid")
            .into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        Name::new("Background"),
    ));
}

fn setup_game_camera(mut commands: Commands) {
    // Game camera for chess board view with Total War-style RTS controls
    // DespawnOnExit automatically despawns this camera when exiting Multiplayer state

    let initial_height = 20.0; // Camera spawn height

    commands.spawn((
        Camera3d::default(),
        Transform::from_matrix(Mat4::from_rotation_translation(
            Quat::from_xyzw(-0.3, -0.5, -0.3, 0.5).normalize(),
            Vec3::new(-7.0, initial_height, 4.0),
        )),
        CameraController {
            current_zoom: initial_height,  // Match initial Y position
            target_zoom: initial_height,   // Match initial Y position
            ..Default::default()           // Use defaults for other fields
        },
        Name::new("Game Camera"),
        DespawnOnExit(GameState::Multiplayer),
    ));
}

/// Preload chess piece GLTF assets during Startup to reduce stack pressure
///
/// This system loads the chess pieces GLTF file during the Startup schedule,
/// before the Multiplayer state transition. This prevents concurrent GLTF parsing
/// in Bevy's Compute Task Pool which can cause stack overflow.
///
/// # Stack Overflow Prevention
///
/// Without preloading, when create_pieces() spawns 32 pieces simultaneously,
/// Bevy's AssetServer triggers 8 concurrent GLTF mesh fragment loads, each
/// performing recursive node traversal in the Compute Task Pool. This exceeds
/// the default 2MB thread stack.
///
/// By preloading here, the GLTF is parsed once in a controlled manner before
/// state transition, and subsequent mesh loads are simple handle clones.
///
/// Reference: `reference/bevy/examples/stress_tests/many_foxes.rs` (spawns 1000+ GLTF models)
fn preload_chess_assets(asset_server: Res<AssetServer>) {
    // Load the main GLTF file - this triggers parsing and caching
    // Subsequent loads in create_pieces() will return cached handles
    asset_server.load::<Gltf>("models/chess_kit/pieces.glb");
    info!("[STARTUP] Preloaded chess piece GLTF assets");
}
