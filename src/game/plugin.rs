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
use super::systems::spectate_sync::SpectateSyncPlugin;
use super::systems::*;
use super::view_mode_systems::*;
use crate::core::{debug_current_gamestate, GameMode, GameState};
use crate::engine::board_state::ChessEngine;
use crate::game::components::{
    FadingCapture, GamePhase, HasMoved, MoveRecord, PieceMoveAnimation, SelectedPiece,
};

use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::ui::game_ui::{
    reset_in_game_hud_visibility, toggle_in_game_hud,
    InGameHudVisibility,
};
use bevy::input::common_conditions::{input_toggle_active, input_just_pressed};
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;

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
            .init_resource::<crate::game::resources::active_time_control::ActiveTimeControl>()
            .init_resource::<CapturedPieces>()
            .init_resource::<GameOverState>()
            .init_resource::<DebugThrottle>()
            .init_resource::<PendingTurnAdvance>()
            .init_resource::<TurnStateContext>()
            .init_resource::<ChessEngine>()
            .init_resource::<Players>()
            .init_resource::<super::systems::camera::CameraRotationState>()
            .init_resource::<super::view_mode::ViewMode>()
            .init_resource::<super::view_mode::PlayerViewPreferences>()
            .init_resource::<PendingPromotion>()
            .init_resource::<GameSounds>()
            .init_resource::<super::camera_modes::CameraViewMode>()
            .init_resource::<super::camera_modes::CinematicSequence>()
            .init_resource::<super::camera_modes::CinematicFadeOverlay>()
            .init_resource::<InGameHudVisibility>()
            .init_resource::<super::systems::input::InGameExitConfirmation>()
            .init_resource::<super::systems::network_move::PendingDrawOffer>()
            .init_resource::<super::systems::network_move::PendingRematchOffer>()
            .init_resource::<crate::ui::game::ChatState>()
            .init_resource::<super::replay::PgnReplayState>();

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
            .register_type::<super::camera_modes::CameraViewMode>()
            .add_message::<PromotionSelected>()
            .add_message::<crate::game::events::MoveMadeEvent>()
            .add_message::<crate::game::events::NetworkMoveEvent>()
            .add_message::<crate::game::events::RemoteMoveApplied>()
            .add_message::<crate::game::events::ResignEvent>()
            .add_message::<crate::game::events::DrawOfferEvent>()
            .add_message::<crate::game::events::DrawResponseEvent>()
            .add_message::<crate::game::events::RematchOfferEvent>()
            .add_message::<crate::game::events::RematchResponseEvent>()
            .add_message::<crate::game::events::FlagTimeoutEvent>();

        // Add AI plugin
        app.add_plugins(AIPlugin);

        // Add network sync plugin for P2P multiplayer
        app.add_plugins(GameSyncPlugin);

        // Add spectator sync plugin
        app.add_plugins(SpectateSyncPlugin);

        // Setup gameplay camera and board scene when entering InGame
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                reset_game_resources,
                initialize_players,
                reset_in_game_hud_visibility,
                reset_in_game_exit_confirmation,
                setup_game_camera,
                setup_game_scene,
            )
                .chain()
                .run_if(not(in_mode(GameMode::PgnReplay))),
        );

        // Setup replay when entering InGame in PgnReplay mode
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                super::replay::setup_replay,
                setup_game_camera,
                setup_game_scene,
            )
                .chain()
                .run_if(in_mode(GameMode::PgnReplay)),
        );

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
                camera_movement_system
                    .in_set(GameSystems::Input)
                    .run_if(super::systems::camera::camera_controls_enabled),
                camera_reset_system.in_set(GameSystems::Input),
                camera_zoom_input_system
                    .in_set(GameSystems::Input)
                    .run_if(super::systems::camera::camera_controls_enabled),
                camera_zoom_system
                    .in_set(GameSystems::Input)
                    .run_if(super::systems::camera::camera_controls_enabled),
                camera_rotation_system
                    .in_set(GameSystems::Input)
                    .run_if(super::systems::camera::camera_controls_enabled),
                camera_mode_cycle_system.in_set(GameSystems::Input),
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
                // update_game_phase is gated on CurrentTurn changing so the shakmaty
                // FEN rebuild and legal-move generation only fire once per move,
                // not every frame (60x/s savings).
                update_game_phase
                    .in_set(GameSystems::Execution)
                    .run_if(|ct: Res<CurrentTurn>| ct.is_changed())
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    }),
                start_timer_when_ready.in_set(GameSystems::Execution).run_if(
                    |view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    },
                ),
                update_game_timer.in_set(GameSystems::Execution).run_if(
                    |view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    },
                ),
                // check_game_over_state is gated on GameOverState changing so it
                // doesn't poll every frame — it only fires when a move sets a
                // terminal condition (checkmate, stalemate, timeout, resign).
                check_game_over_state
                    .in_set(GameSystems::Execution)
                    .run_if(|go: Res<GameOverState>| go.is_changed())
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    }),
                crate::game::systems::network_move::handle_resign_events
                    .in_set(GameSystems::Execution),
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
                // highlight_possible_moves is gated on Selection changing so the
                // 64-square iteration and material handle clones only happen when a
                // piece is clicked or a move is made (not 60x/s on idle frames).
                highlight_possible_moves
                    .in_set(GameSystems::Visual)
                    .run_if(|sel: Res<Selection>| sel.is_changed())
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| {
                        *view_mode != super::view_mode::ViewMode::TempleOS
                    }),
                // animate_piece_movement is skipped entirely when no piece has a
                // PieceMoveAnimation component (archetype cache lookup — zero cost).
                animate_piece_movement.in_set(GameSystems::Visual),
                // animate_capture_fade is skipped when nothing is mid-fade.
                animate_capture_fade
                    .in_set(GameSystems::Visual)
                    .run_if(any_with_component::<FadingCapture>),
            ),
        );

        // Egui UI systems must run in EguiPrimaryContextPass (bevy_egui 0.39+)
        // so that button clicks and other pointer interactions are received.
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            (
                crate::ui::game::game_ui::game_status_ui,
                crate::ui::game::game_ui::draw_offer_ui,
                crate::ui::game::game_ui::rematch_offer_ui,
                crate::ui::game::game_ui::post_game_overlay,
                crate::ui::game::game_ui::pause_resume_ui,
                crate::ui::game_2d::render_2d_board,
            )
                .chain()
                .run_if(in_state(GameState::InGame))
                .run_if(not(in_mode(GameMode::PgnReplay))),
        );
        // Chat panel runs separately (shares EguiContexts with the chain above;
        // Bevy allows multiple systems in the same pass to use EguiContexts as long
        // as they are not chained with conflicting system-params).
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            crate::ui::game::chat_panel_ui
                .run_if(in_state(GameState::InGame))
                .run_if(not(in_mode(GameMode::PgnReplay))),
        );

        // Item 3: session key expiry banner
        #[cfg(feature = "solana")]
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            crate::ui::game::game_ui::session_expiry_banner
                .run_if(in_state(GameState::InGame)),
        );

        // Replay UI overlay
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            super::replay::replay_ui_system
                .run_if(in_state(GameState::InGame))
                .run_if(in_mode(GameMode::PgnReplay)),
        );

        // Draw offer / rematch / chat network handlers
        app.add_systems(
            Update,
            (
                crate::game::systems::network_move::watch_draw_offers,
                crate::game::systems::network_move::handle_draw_response_events,
                crate::game::systems::network_move::watch_rematch_offers,
                crate::ui::game::drain_chat_messages,
            )
                .in_set(GameSystems::Execution)
                .run_if(in_state(GameState::InGame)),
        );

        // Conditional 3D visibility system
        app.add_systems(
            Update,
            (
                toggle_3d_visibility,
                toggle_in_game_hud,
                confirm_exit_game,
            )
                .run_if(in_state(GameState::InGame)),
        );

        // Cinematic camera must run in both InGame and GameOver so the game-over
        // cinematic actually ticks. InGameplay covers InGame + Paused + GameOver.
        app.add_systems(
            Update,
            cinematic_camera_system.run_if(crate::core::in_gameplay),
        );

        // Debug system - toggle with F12 key
        app.add_systems(
            Update,
            debug_current_gamestate.run_if(input_toggle_active(false, KeyCode::F12)),
        );

        // Fullscreen toggle - F11 key
        app.add_systems(
            Update,
            toggle_fullscreen.run_if(input_just_pressed(KeyCode::F11)),
        );

        // ESC key to exit to main menu (forfeit/leave game)
        app.add_systems(
            Update,
            handle_escape_key.run_if(in_state(GameState::InGame)),
        );

        // Global visual setup
        app.add_systems(Startup, setup_global_scene);

        // Add mesh picking plugin for 3D picking support (required in Bevy 0.18)
        app.add_plugins(MeshPickingPlugin);

        app.add_systems(OnExit(GameState::InGame), (reset_game_camera,));

        // Replay cleanup
        app.add_systems(
            OnExit(GameState::InGame),
            super::replay::cleanup_replay.run_if(in_mode(GameMode::PgnReplay)),
        );

        // Replay playback systems (run every frame during InGame + PgnReplay)
        app.add_systems(
            Update,
            (
                super::replay::replay_auto_advance_system,
                super::replay::replay_apply_move_system,
                super::replay::replay_sync_engine_system,
                super::replay::replay_spawn_pieces_system,
            )
                .chain()
                .run_if(in_state(GameState::InGame))
                .run_if(in_mode(GameMode::PgnReplay)),
        );
    }
}

/// Run condition: current game mode matches `mode`.
pub fn in_mode(mode: crate::core::states::GameMode) -> impl Fn(Res<crate::core::states::GameMode>) -> bool {
    move |game_mode: Res<crate::core::states::GameMode>| *game_mode == mode
}

/// Run condition: current game mode is NOT `mode`.
pub fn not_in_mode(mode: crate::core::states::GameMode) -> impl Fn(Res<crate::core::states::GameMode>) -> bool {
    move |game_mode: Res<crate::core::states::GameMode>| *game_mode != mode
}
