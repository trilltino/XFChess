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
use crate::core::{debug_current_gamestate, GameMode, GameState};
use crate::engine::board_state::ChessEngine;
use crate::game::components::{
    FadingCapture, GamePhase, HasMoved, MoveRecord, PieceMoveAnimation, SelectedPiece,
};

use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::ui::game_2d::Board2DTheme;
use crate::ui::game_ui::{
    reset_in_game_hud_visibility, toggle_in_game_hud, InGameHudVisibility, IncrementFlash,
};
use bevy::input::common_conditions::{input_just_pressed, input_toggle_active};
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
            .init_resource::<PendingPromotion>()
            .init_resource::<GameSounds>()
            .init_resource::<MenuSounds>()
            .init_resource::<super::camera_modes::CameraViewMode>()
            .init_resource::<super::camera_modes::CinematicSequence>()
            .init_resource::<super::camera_modes::CinematicFadeOverlay>()
            .init_resource::<InGameHudVisibility>()
            .init_resource::<IncrementFlash>()
            .init_resource::<Board2DTheme>()
            .init_resource::<super::systems::input::InGameExitConfirmation>()
            .init_resource::<super::systems::network_move::PendingDrawOffer>()
            .init_resource::<super::systems::network_move::PendingRematchOffer>()
            .init_resource::<crate::ui::game::ChatState>()
            .init_resource::<super::replay::PgnReplayState>()
            .init_resource::<crate::ui::game::game_ui::TimeoutHourglassState>()
            .init_resource::<crate::ui::game::game_ui::AvatarCache>();

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

        // 30-second first-move grace period (online games only)
        super::systems::first_move_timer::register(app);

        // Clips the dedicated board camera's viewport to the board column
        // between the fixed-width left/right egui side panels.
        app.add_systems(
            Update,
            super::systems::camera::sync_board_camera_viewport
                .run_if(in_state(GameState::InGame)),
        );

        // Restore ambient when game ends so the board behind the popup looks neutral.
        app.add_systems(
            OnEnter(GameState::GameOver),
            |mut global_ambient: ResMut<bevy::light::GlobalAmbientLight>| {
                global_ambient.color = Color::srgb(0.82, 0.87, 1.0);
                global_ambient.brightness = 800.0;
            },
        );

        // Setup gameplay camera and board scene when entering InGame
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                purge_stale_board_visuals,
                reset_game_resources,
                initialize_players,
                reset_in_game_hud_visibility,
                reset_in_game_exit_confirmation,
                setup_game_camera,
                setup_game_scene,
                super::systems::game_init::warmup_game_audio,
            )
                .chain()
                .run_if(not(in_mode(GameMode::PgnReplay))),
        );

        // Setup replay when entering InGame in PgnReplay mode
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                purge_stale_board_visuals,
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
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
                camera_rotate_on_turn_system
                    .in_set(GameSystems::Input)
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
                // Validation set: Sync board state before validation (disabled in TempleOS)

                // Execution set: Update game state (disabled in TempleOS)
                // Advance the turn immediately (before AI runs) so the
                // AI sees the new turn and responds in the same frame the player moved.
                flush_pending_turn.in_set(GameSystems::Execution),
                // Run when the turn changes (normal per-move path) OR when the
                // legal-move cache is still empty but the game is not yet over.
                // The second condition catches the frame after deferred piece-spawn
                // commands are flushed: CurrentTurn has not changed, but the cache
                // is empty and needs to be built for the first time.
                update_game_phase
                    .in_set(GameSystems::Execution)
                    .run_if(
                        |ct: Res<CurrentTurn>,
                         engine: Res<crate::engine::board_state::ChessEngine>,
                         game_over: Res<super::resources::GameOverState>| {
                            ct.is_changed()
                                || (!engine.has_legal_moves() && !game_over.is_game_over())
                        },
                    )
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
                start_timer_when_ready
                    .in_set(GameSystems::Execution)
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
                update_game_timer
                    .in_set(GameSystems::Execution)
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
                // check_game_over_state is gated on GameOverState changing so it
                // doesn't poll every frame — it only fires when a move sets a
                // terminal condition (checkmate, stalemate, timeout, resign).
                check_game_over_state
                    .in_set(GameSystems::Execution)
                    .run_if(|go: Res<GameOverState>| go.is_changed())
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
                crate::game::systems::network_move::handle_resign_events
                    .in_set(GameSystems::Execution),
                crate::game::systems::network_move::handle_flag_timeout_events
                    .in_set(GameSystems::Execution),
                // Promotion detection and handling (disabled in TempleOS)
                detect_pawn_promotion
                    .in_set(GameSystems::Execution)
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
                apply_pawn_promotion
                    .in_set(GameSystems::Execution)
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
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
                    .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
                // animate_piece_movement is skipped entirely when no piece has a
                // PieceMoveAnimation component (archetype cache lookup — zero cost).
                // Nested to stay under Bevy's tuple-arity limit for `.chain()`
                // (the flat list above this point already has 19 systems) —
                // this sub-tuple is itself chained, so overall ordering is
                // unchanged from a flat 21-element chain.
                (
                    animate_piece_movement.in_set(GameSystems::Visual),
                    // animate_capture_fade is skipped when nothing is mid-fade.
                    animate_capture_fade
                        .in_set(GameSystems::Visual)
                        .run_if(any_with_component::<FadingCapture>),
                )
                    .chain(),
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
                crate::ui::game_2d::render_2d_board,
                crate::ui::game::promotion_ui::promotion_ui_system,
            )
                .chain()
                .run_if(in_state(GameState::InGame))
                .run_if(not(in_mode(GameMode::PgnReplay))),
        );
        // 2D board arrow overlays, drag-to-move, premove, piece animation
        app.init_resource::<crate::ui::game::game_2d::BoardArrows>();
        app.init_resource::<crate::ui::game::game_2d::DragState2D>();
        app.init_resource::<crate::ui::game::game_2d::PremoveState>();
        app.init_resource::<crate::ui::game::game_2d::PieceAnim2D>();
        app.add_systems(
            Update,
            crate::ui::game::game_2d::trigger_piece_anim_2d.run_if(in_state(GameState::InGame)),
        );

        // Eval bar resources and update system
        app.init_resource::<crate::ui::game::game_2d::EvalBarState>();
        app.init_resource::<crate::ui::game::game_2d::EvalHistory>();
        app.init_resource::<crate::ui::game::game_2d::BoardFocus>();
        app.init_resource::<crate::ui::game::game_2d::CheckmateFlashState>();
        app.init_resource::<crate::ui::game::game_2d::BoardFadeState>();
        app.add_systems(
            Update,
            (
                crate::ui::game::game_2d::trigger_checkmate_flash,
                crate::ui::game::game_2d::tick_checkmate_flash,
                crate::ui::game::game_2d::board_fade_system,
            )
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            crate::ui::game::game_2d::update_eval_bar.run_if(in_state(GameState::InGame)),
        );

        // Sync Board2DTheme and eval bar visibility from GameSettings on settings change
        app.add_systems(
            Update,
            crate::ui::game::game_2d::sync_board_theme_from_settings,
        );
        app.add_systems(Update, crate::ui::game::game_2d::sync_eval_bar_visibility);
        app.add_systems(
            Update,
            crate::rendering::pieces::pieces::reload_piece_sprites,
        );

        // Keep the board fill light on the camera so pieces stay evenly lit from
        // the viewer's side as the camera orbits/zooms (even lighting at any angle).
        app.add_systems(
            Update,
            super::systems::visual::update_board_fill_light
                .run_if(|view_mode: Res<super::view_mode::ViewMode>| !view_mode.is_templeos()),
        );

        // Ping chip (online games only)
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            crate::ui::game::game_ui::ping_chip_ui.run_if(in_state(GameState::InGame)),
        );

        // Opponent disconnect popup + countdown
        app.init_resource::<crate::ui::game::game_ui::OpponentDisconnectState>();
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            crate::ui::game::game_ui::opponent_disconnect_ui.run_if(in_state(GameState::InGame)),
        );

        // Timeout hourglass — tracks FlagTimeoutEvent and drives animated ⧖ in timer chip
        app.add_systems(
            Update,
            crate::ui::game::game_ui::timeout_hourglass_system.run_if(in_state(GameState::InGame)),
        );

        // Player avatar fetch — background HTTP → Bevy Image asset
        app.add_systems(
            Update,
            crate::ui::game::game_ui::avatar_fetch_system.run_if(in_state(GameState::InGame)),
        );

        // Check sound cue
        app.add_systems(
            Update,
            crate::ui::game::game_ui::play_check_sound_system.run_if(in_state(GameState::InGame)),
        );

        // Blindfold toggle — Ctrl+B
        app.add_systems(
            Update,
            crate::ui::game::game_ui::toggle_blindfold_system.run_if(in_state(GameState::InGame)),
        );

        // Tournament sidebar widget — shown when active_tournament_id is set
        #[cfg(feature = "solana")]
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            crate::ui::game::game_ui::tournament_sidebar_widget.run_if(in_state(GameState::InGame)),
        );

        // Increment flash tick — pulses +Xs label after each move
        app.add_systems(
            Update,
            crate::ui::game::game_ui::increment_flash_system.run_if(in_state(GameState::InGame)),
        );

        // Chat now renders inline inside game_status_ui's left panel (see
        // crate::ui::game::left_panel) — no standalone system needed here.

        // Item 3: session key expiry banner
        #[cfg(feature = "solana")]
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            crate::ui::game::game_ui::session_expiry_banner.run_if(in_state(GameState::InGame)),
        );

        // Disconnect recovery banner (online modes only)
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            crate::ui::game::game_ui::disconnect_recovery_banner
                .run_if(in_state(GameState::InGame))
                .run_if(not(in_mode(GameMode::PgnReplay))),
        );

        // Replay UI overlay
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            super::replay::replay_ui_system
                .run_if(in_state(GameState::InGame))
                .run_if(in_mode(GameMode::PgnReplay)),
        );

        // ── Shorts creation features ──
        app.init_resource::<super::replay_shorts::ReplayAnnotations>();
        app.init_resource::<super::replay_shorts::PuzzleOverlay>();
        app.init_resource::<super::replay_shorts::CinematicEffect>();
        app.init_resource::<super::replay_shorts::QualityBadgeState>();
        app.init_resource::<super::shorts_state::ShortsState>();
        app.init_resource::<crate::rendering::camera::camera_director::CameraDirector>();
        app.add_message::<super::replay_shorts::ScreenshotRequested>();
        app.add_message::<super::replay_shorts::BlunderFlash>();
        app.add_message::<super::replay_shorts::BrilliantGlow>();
        app.add_message::<super::replay_shorts::CheckmateFlash>();

        // 2D board + annotation overlay (only in PgnReplay + Standard2D)
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            super::replay_shorts::replay_2d_annotation_system
                .run_if(in_state(GameState::InGame))
                .run_if(in_mode(GameMode::PgnReplay)),
        );

        // Cinematic flash + quality badge + hook text overlays
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            (
                super::replay_shorts::cinematic_effect_system,
                super::replay_shorts::quality_badge_system,
                super::replay_shorts::hook_text_system,
            )
                .run_if(in_state(GameState::InGame))
                .run_if(in_mode(GameMode::PgnReplay)),
        );

        // 3D mesh arrows (rebuild on annotation change in 3D mode)
        app.add_systems(
            Update,
            (
                super::replay_shorts::replay_3d_annotations_system,
                super::replay_shorts::clear_annotations_on_ply_change,
                super::replay_shorts::replay_screenshot_system,
            )
                .run_if(in_state(GameState::InGame))
                .run_if(in_mode(GameMode::PgnReplay)),
        );

        // Shorts cinematic tick + capture sequence (always run in replay mode)
        app.add_systems(
            Update,
            (
                super::replay_shorts::cinematic_tick_system,
                super::replay_shorts::capture_sequence_system,
            )
                .run_if(in_state(GameState::InGame))
                .run_if(in_mode(GameMode::PgnReplay)),
        );

        // Camera director (always run in replay mode)
        app.add_systems(
            Update,
            crate::rendering::camera::camera_director::camera_director_system
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

        app.add_systems(
            Update,
            (toggle_in_game_hud, confirm_exit_game).run_if(in_state(GameState::InGame)),
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

        // F11 hint overlay (bottom-right, visible only when fullscreen)
        app.add_systems(Update, render_fullscreen_hint);

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
                super::replay_shorts::load_pgn_annotations_system,
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
pub fn in_mode(
    mode: crate::core::states::GameMode,
) -> impl Fn(Res<crate::core::states::GameMode>) -> bool {
    move |game_mode: Res<crate::core::states::GameMode>| *game_mode == mode
}

/// Run condition: current game mode is NOT `mode`.
pub fn not_in_mode(
    mode: crate::core::states::GameMode,
) -> impl Fn(Res<crate::core::states::GameMode>) -> bool {
    move |game_mode: Res<crate::core::states::GameMode>| *game_mode != mode
}
