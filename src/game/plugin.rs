//! Game plugin - Core chess game logic and systems.

//! Systems are organized into sets with explicit ordering:
//!
//! Systems are organized into sets with explicit ordering:
//! - `Input` - Handle user input (camera, piece selection)
//! - `Validation` - Validate moves and sync board state
//! - `Execution` - Execute moves and update game state
//! - `Visual` - Update rendering (highlights, animations)
//!
//! System execution order is controlled via [`GameSystems`] sets and `.chain()`.
//!
//! # Resources
//!
//! All game resources are initialized here. See [`super::resources`] for details.
//!
//! # See Also
//!
//! - [`super::resources`] - Game resource definitions
//! - [`super::systems`] - Game system implementations
//! - [`super::system_sets`] - System set definitions
//! - [`crate::core::CorePlugin`] - Core plugin that must be added first

use super::ai::AIPlugin;
use super::resources::*;
use super::sync::GameSyncPlugin;
use super::system_sets::GameSystems;
use super::systems::picking_debug::PickingDebugPlugin;
use super::systems::*;
use crate::core::{debug_current_gamestate, GameState};
use crate::engine::board_state::ChessEngine;
use crate::game::components::{
    FadingCapture, GamePhase, HasMoved, MoveRecord, PieceMoveAnimation, SelectedPiece,
};

use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::ui::game_ui::game_status_ui;
use crate::ui::promotion_ui::promotion_ui_system;
use bevy::input::common_conditions::{input_toggle_active, input_just_pressed};
use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;

fn test_event_system(mut reader: MessageReader<crate::game::events::MoveMadeEvent>) {
    for _ in reader.read() {}
}

/// Game plugin for XFChess
///
/// Registers all game systems and resources. This plugin should be added
/// after CorePlugin and before state-specific plugins.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        // Register resources
        app.init_resource::<CurrentTurn>()
            .init_resource::<CurrentGamePhase>()
            .init_resource::<Selection>()
            .init_resource::<MoveHistory>()
            .init_resource::<GameTimer>()
            .init_resource::<CapturedPieces>()
            .init_resource::<GameOverState>()
            .init_resource::<DebugThrottle>()
            .init_resource::<PendingTurnAdvance>()
            .init_resource::<TurnStateContext>()
            .init_resource::<ChessEngine>()
            .init_resource::<Players>()
            .init_resource::<super::systems::camera::CameraRotationState>()
            .init_resource::<super::view_mode::ViewMode>()
            .init_resource::<PendingPromotion>()
            .init_resource::<GameSounds>();

        // Register types for reflection (needed for inspector)
        app.register_type::<CurrentTurn>()
            .register_type::<CurrentGamePhase>()
            .register_type::<GameTimer>()
            .register_type::<MoveHistory>()
            .register_type::<CapturedPieces>()
            .register_type::<GameOverState>()
            .register_type::<PendingTurnAdvance>()
            .register_type::<TurnStateContext>()
            .register_type::<TurnPhase>()
            .register_type::<GamePhase>()
            .register_type::<MoveRecord>()
            .register_type::<Piece>()
            .register_type::<PieceColor>()
            .register_type::<PieceType>()
            .register_type::<HasMoved>()
            .register_type::<PieceMoveAnimation>()
            .register_type::<FadingCapture>()
            .register_type::<SelectedPiece>()
            .register_type::<CameraController>()
            .register_type::<Player>()
            .register_type::<Players>()
            .register_type::<super::view_mode::ViewMode>()
            .add_message::<PromotionSelected>()
            .add_message::<crate::game::events::MoveMadeEvent>()
            .add_message::<crate::game::events::NetworkMoveEvent>()
            .add_message::<crate::game::events::ResignEvent>()
            .add_message::<crate::game::events::DrawOfferEvent>()
            .add_message::<crate::game::events::DrawResponseEvent>();

        // Add AI plugin
        app.add_plugins(AIPlugin);

        // Add network sync plugin for P2P multiplayer
        app.add_plugins(GameSyncPlugin);

        // Configure system sets to run in order: Input → Validation → Execution → Visual
        app.configure_sets(
            Update,
            (
                GameSystems::Input,
                GameSystems::Validation,
                GameSystems::Execution,
                GameSystems::Visual,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );

        // Register systems with run conditions
        // Systems are assigned to sets for predictable execution order
        // NOTE: Input handling is now done via observers on entities (.observe())
        // so we don't need handle_piece_selection/clear_selection_on_empty_click systems
        // NOTE: Game logic systems are disabled in TempleOS mode (just a board, no game)
        app.add_systems(
            Update,
            (
                // Input set: Handle user input (camera only in TempleOS)
                camera_movement_system.in_set(GameSystems::Input),
                camera_reset_system.in_set(GameSystems::Input),
                camera_zoom_input_system.in_set(GameSystems::Input),
                camera_zoom_system.in_set(GameSystems::Input),
                camera_rotation_system.in_set(GameSystems::Input),
                camera_rotate_on_turn_detection_system
                    .in_set(GameSystems::Input)
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    }),
                camera_rotate_on_turn_system
                    .in_set(GameSystems::Input)
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    }),
                // Validation set: Sync board state before validation (disabled in TempleOS)

                // Execution set: Update game state (disabled in TempleOS)
                update_game_phase.in_set(GameSystems::Execution).run_if(
                    |view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    },
                ),
                update_game_timer.in_set(GameSystems::Execution).run_if(
                    |view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    },
                ),
                check_game_over_state.in_set(GameSystems::Execution).run_if(
                    |view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    },
                ),
                // Promotion detection and handling (disabled in TempleOS)
                detect_pawn_promotion.in_set(GameSystems::Execution).run_if(
                    |view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    },
                ),
                apply_pawn_promotion.in_set(GameSystems::Execution).run_if(
                    |view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    },
                ),
                // Network Move Verification/Execution
                crate::game::systems::network_move::handle_network_moves
                    .in_set(GameSystems::Execution),
                // Visual set: Update rendering (disabled in TempleOS)
                highlight_possible_moves.in_set(GameSystems::Visual).run_if(
                    |view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    },
                ),
                animate_piece_movement.in_set(GameSystems::Visual).run_if(
                    |view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    },
                ),
                // Capture fade removed — captured pieces now despawn immediately
            ),
        );

        // Add UI system separately (egui requires EguiPrimaryContextPass)
        app.add_systems(
            EguiPrimaryContextPass,
            (game_status_ui, promotion_ui_system).run_if(in_state(GameState::InGame)),
        );

        // Debug system - toggle with F12 key
        app.add_systems(
            Update,
            debug_current_gamestate.run_if(input_toggle_active(true, KeyCode::F12)),
        );

        // Fullscreen toggle - F11 key
        app.add_systems(
            Update,
            toggle_fullscreen.run_if(input_just_pressed(KeyCode::F11)),
        );

        // Global visual setup
        app.add_systems(Startup, setup_global_scene);

        // Add mesh picking plugin for 3D picking support (required in Bevy 0.18)
        app.add_plugins(MeshPickingPlugin);

        // Add picking debug plugin
        app.add_plugins(PickingDebugPlugin);

        // InGame setup systems
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                reset_game_resources.before(crate::rendering::pieces::create_pieces),
                initialize_players,
                setup_game_scene,
                setup_game_camera,
                initialize_engine_from_ecs.after(crate::rendering::pieces::create_pieces),
            ),
        );

        app.add_systems(OnExit(GameState::InGame), (reset_game_camera,));

        // This must run after pieces are created, we can schedule it in Update temporarily
        // or a different state, but let's just use PostStartup or a delayed system?
        // Wait, PiecePlugin probably spawns pieces on OnEnter(GameState::InGame).
        // If we add this to OnEnter(GameState::InGame), we should order it after piece spawning.
        // Piece spawning is in `crate::rendering::pieces::create_pieces`.
        // Let's just add it to OnEnter(GameState::InGame) for now, Bevy's default ordering might be enough,
        // or we can use `.after(crate::rendering::pieces::create_pieces)`.
    }
}
