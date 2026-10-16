use super::ai::AIPlugin;
use super::resources::*;
use super::sync::GameSyncPlugin;
use super::systems::spectate_sync::SpectateSyncPlugin;
use super::systems::*;
use crate::core::schedule::GameSystems;
use crate::core::{debug_current_gamestate, GameMode, GameState, InGameplay};
use crate::engine::board_state::ChessEngine;
use crate::game::components::{
    FadingCapture, GamePhase, HasMoved, MoveRecord, PieceMoveAnimation, SelectedPiece,
};
use crate::input::board::{
    confirm_exit_game, handle_escape_key, reset_in_game_exit_confirmation, toggle_fullscreen,
};

use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::ui::game_2d::Board2DTheme;
use crate::ui::game_ui::{
    reset_in_game_hud_visibility, toggle_in_game_hud, InGameHudVisibility, IncrementFlash,
};
use bevy::input::common_conditions::{input_just_pressed, input_toggle_active};
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;

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
            .init_resource::<PendingGameOver>()
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
            .init_resource::<super::camera_modes::CameraLockState>()
            .init_resource::<InGameHudVisibility>()
            .init_resource::<IncrementFlash>()
            .init_resource::<Board2DTheme>()
            .init_resource::<crate::input::board::InGameExitConfirmation>()
            .init_resource::<super::systems::network_move::PendingDrawOffer>()
            .init_resource::<super::systems::network_move::PendingRematchOffer>()
            .init_resource::<crate::ui::game::ChatState>()
            .init_resource::<super::replay::PgnReplayState>()
            .init_resource::<crate::ui::game::game_ui::TimeoutHourglassState>()
            .init_resource::<crate::ui::game::game_ui::AvatarCache>()
            .init_resource::<crate::ui::game::player_bar::PlayerBarsCache>()
            .init_resource::<crate::ui::game::player_bar::PlayerBarLayoutCache>();

        // Register types for reflection.
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
            .register_type::<super::camera_modes::CameraLockState>()
            .add_message::<PromotionSelected>()
            .add_message::<crate::game::events::MoveMadeEvent>()
            .add_message::<crate::game::events::NetworkMoveEvent>()
            .add_message::<crate::game::events::RemoteMoveApplied>()
            .add_message::<crate::game::events::NetworkMoveResult>()
            .add_message::<crate::game::events::ResignEvent>()
            .add_message::<crate::game::events::DrawOfferEvent>()
            .add_message::<crate::game::events::DrawResponseEvent>()
            .add_message::<crate::game::events::RematchOfferEvent>()
            .add_message::<crate::game::events::RematchResponseEvent>()
            .add_message::<crate::game::events::FlagTimeoutEvent>();

        app.add_plugins(AIPlugin);

        app.add_plugins(GameSyncPlugin);

        app.add_plugins(SpectateSyncPlugin);
        #[cfg(debug_assertions)]
        app.add_plugins(super::systems::picking_debug::PickingDebugPlugin);

        // 30-second first-move grace period (online games only)
        super::systems::first_move_timer::register(app);

        // Restore ambient when game ends so the board behind the popup looks neutral.
        app.add_systems(
            OnEnter(GameState::GameOver),
            |mut global_ambient: ResMut<bevy::light::GlobalAmbientLight>| {
                global_ambient.color = Color::srgb(0.82, 0.87, 1.0);
                global_ambient.brightness = 800.0;
            },
        );

        // Clear the pending game-over prompt whenever a (new) game starts or the
        // stats screen is entered, so a stale prompt never leaks across games.
        app.add_systems(
            OnEnter(GameState::InGame),
            |mut pending: ResMut<PendingGameOver>| pending.active = false,
        );
        app.add_systems(
            OnEnter(GameState::GameOver),
            |mut pending: ResMut<PendingGameOver>| pending.active = false,
        );

        // Setup gameplay camera and board scene when entering InGame
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                super::ai::systems::clear_pending_ai_tasks,
                purge_stale_board_visuals,
                reset_game_resources,
                initialize_players,
                reset_in_game_hud_visibility,
                reset_in_game_exit_confirmation,
                setup_game_camera,
                setup_game_scene,
                super::systems::game_init::warmup_game_audio,
                super::systems::game_init::spawn_game_lights,
            )
                .chain()
                .run_if(not(resource_equals(GameMode::PgnReplay))),
        );

        // Setup replay when entering InGame in PgnReplay mode
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                super::ai::systems::clear_pending_ai_tasks,
                purge_stale_board_visuals,
                super::replay::setup_replay,
                setup_game_camera,
                setup_game_scene,
                super::systems::game_init::spawn_game_lights,
            )
                .chain()
                .run_if(resource_equals(GameMode::PgnReplay)),
        );

        // Frame order (configured in core::schedule): Input → Validation → Execution
        // → Visual, all InGame-only. Each phase below is one chain, so systems that
        // share game state always run in the order written. Selection itself is
        // handled by picking observers (crate::input::board).

        // Keys, then camera: pan/zoom/rotate input, smoothing, auto-rotate on turn.
        app.add_systems(
            Update,
            (
                handle_escape_key,
                confirm_exit_game,
                toggle_in_game_hud,
                crate::ui::game::game_ui::toggle_blindfold_system,
                #[cfg(not(target_os = "android"))]
                camera_movement_system.run_if(camera_controls_enabled),
                // Android touch gestures replace desktop pan, drag and scroll; both feed
                // CameraController.target_zoom and share the smoothing system.
                #[cfg(target_os = "android")]
                camera_touch_gestures.run_if(camera_controls_enabled),
                camera_reset_system,
                #[cfg(not(target_os = "android"))]
                camera_zoom_input_system.run_if(camera_controls_enabled),
                camera_zoom_system.run_if(camera_controls_enabled),
                #[cfg(not(target_os = "android"))]
                camera_rotation_system.run_if(camera_controls_enabled),
                camera_rotate_on_turn_detection_system.run_if(not_templeos),
                camera_rotate_on_turn_system.run_if(not_templeos),
            )
                .chain()
                .after(crate::game::systems::camera::view_mode_toggle_input_system)
                .in_set(GameSystems::Input),
        );

        // Replay playback feeds its moves in before the game logic runs.
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
                .in_set(crate::core::schedule::MoveSource::Replay)
                .run_if(resource_equals(GameMode::PgnReplay)),
        );

        // Apply this frame's moves, then derive everything that follows from them:
        // turn, check/mate, clocks, resign/timeout/draw, and the game-over prompt.
        // The AI (game::ai) thinks after this chain, on the final state.
        app.add_systems(
            Update,
            (
                crate::game::systems::network_move::handle_network_moves,
                apply_pawn_promotion.run_if(not_templeos),
                detect_pawn_promotion.run_if(not_templeos),
                flush_pending_turn,
                // An empty cache also triggers after deferred piece spawns, even when
                // CurrentTurn has not changed.
                update_game_phase
                    .run_if(
                        |ct: Res<CurrentTurn>,
                         engine: Res<crate::engine::board_state::ChessEngine>,
                         game_over: Res<super::resources::GameOverState>| {
                            ct.is_changed()
                                || (!engine.has_legal_moves() && !game_over.is_game_over())
                        },
                    )
                    .run_if(not_templeos),
                start_timer_when_ready.run_if(not_templeos),
                update_game_timer.run_if(not_templeos),
                crate::game::systems::network_move::handle_resign_events,
                crate::game::systems::network_move::handle_flag_timeout_events,
                crate::game::systems::network_move::watch_draw_offers,
                crate::game::systems::network_move::handle_draw_response_events,
                crate::game::systems::network_move::watch_rematch_offers,
                crate::ui::game::drain_chat_messages,
                // Keep checking until move and capture animations finish; a
                // change-only gate would miss completion on a later frame.
                check_game_over_state
                    .run_if(|go: Res<GameOverState>| go.is_game_over())
                    .run_if(not_templeos),
                confirm_game_over_prompt,
            )
                .chain()
                .in_set(GameSystems::Execution),
        );

        // Draw the result of the frame: highlights and animations, replay camera
        // work, then the HUD effects and lights that follow the camera.
        app.add_systems(
            Update,
            (
                // Rebuild highlights only when Selection changes.
                highlight_possible_moves
                    .run_if(|sel: Res<Selection>| sel.is_changed())
                    .run_if(not_templeos),
                animate_piece_movement,
                animate_capture_fade.run_if(any_with_component::<FadingCapture>),
                crate::ui::game::game_2d::trigger_piece_anim_2d,
                crate::ui::game::game_2d::trigger_checkmate_flash,
                crate::ui::game::game_2d::tick_checkmate_flash,
                crate::ui::game::game_2d::board_fade_system,
                crate::ui::game::game_2d::update_eval_bar,
                crate::ui::game::game_ui::timeout_hourglass_system,
                crate::ui::game::game_ui::increment_flash_system,
                crate::ui::game::game_ui::play_check_sound_system,
                crate::ui::game::game_ui::avatar_fetch_system,
                // Keep the board fill light on the camera so pieces stay evenly lit
                // from the viewer's side as the camera orbits and zooms.
                super::systems::visual::update_board_fill_light.run_if(not_templeos),
            )
                .chain()
                .in_set(GameSystems::Visual),
        );
        // Replay-only visuals: annotations, shorts capture and the camera director,
        // after pieces settle and before the fill light follows the camera.
        app.add_systems(
            Update,
            (
                super::replay_shorts::replay_3d_annotations_system,
                super::replay_shorts::clear_annotations_on_ply_change,
                super::replay_shorts::replay_screenshot_system,
                super::replay_shorts::cinematic_tick_system,
                super::replay_shorts::capture_sequence_system,
                crate::rendering::camera::camera_director::camera_director_system,
            )
                .chain()
                .after(animate_capture_fade)
                .before(super::systems::visual::update_board_fill_light)
                .in_set(GameSystems::Visual)
                .run_if(resource_equals(GameMode::PgnReplay)),
        );

        // Egui UI systems must run in EguiPrimaryContextPass (bevy_egui 0.39+)
        // so that button clicks and other pointer interactions are received.
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            (
                crate::ui::game::game_ui::game_status_ui,
                crate::ui::game::game_ui::draw_offer_ui,
                crate::ui::game::game_ui::rematch_offer_ui,
                crate::ui::game_2d::render_2d_board,
                crate::ui::game::promotion_ui::promotion_ui_system,
            )
                .chain()
                .run_if(in_state(GameState::InGame))
                .run_if(not(resource_equals(GameMode::PgnReplay))),
        );
        // 2D board arrow overlays, drag-to-move, premove, piece animation
        app.init_resource::<crate::ui::game::game_2d::BoardArrows>();
        app.init_resource::<crate::ui::game::game_2d::DragState2D>();
        app.init_resource::<crate::ui::game::game_2d::PremoveState>();
        app.init_resource::<crate::ui::game::game_2d::PieceAnim2D>();
        app.init_resource::<crate::ui::game::game_2d::Board2DLayout>();
        app.init_resource::<crate::ui::game::game_2d::EvalBarState>();
        app.init_resource::<crate::ui::game::game_2d::EvalHistory>();
        app.add_systems(
            OnEnter(crate::core::GameState::InGame),
            crate::ui::game::game_2d::reset_evaluation,
        );
        app.add_systems(
            OnExit(crate::core::states::InGameplay),
            crate::ui::game::game_2d::reset_evaluation,
        );

        app.init_resource::<crate::ui::game::game_2d::BoardFocus>();
        app.init_resource::<crate::ui::game::game_2d::CheckmateFlashState>();
        app.init_resource::<crate::ui::game::game_2d::BoardFadeState>();
        app.init_resource::<crate::ui::game::game_ui::OpponentDisconnectState>();

        // Mirror settings into the 2D board theme, eval bar and piece sprites when they
        // change (the first frame after loading counts, so sprites load at startup).
        app.add_systems(
            Update,
            (
                crate::ui::game::game_2d::sync_board_theme_from_settings,
                crate::ui::game::game_2d::sync_eval_bar_visibility,
                crate::rendering::pieces::reload_piece_sprites,
            )
                .chain()
                .in_set(crate::core::schedule::ApplySettings)
                .run_if(resource_exists_and_changed::<crate::core::GameSettings>),
        );

        // In-game overlays drawn in the egui pass.
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            (
                crate::ui::game::game_ui::ping_chip_ui,
                crate::ui::game::game_ui::opponent_disconnect_ui,
                #[cfg(feature = "solana")]
                crate::ui::game::game_ui::tournament_sidebar_widget,
                (
                    crate::ui::game::game_ui::disconnect_recovery_banner,
                    // "Press Enter to see game stats" while the final position shows.
                    crate::ui::game::game_ui::game_over_prompt_banner,
                )
                    .run_if(not(resource_equals(GameMode::PgnReplay))),
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );

        // Replay overlays.
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
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            (
                super::replay::replay_ui_system,
                // 2D board + annotation overlay (Standard2D view).
                super::replay_shorts::replay_2d_annotation_system,
                super::replay_shorts::cinematic_effect_system,
                super::replay_shorts::quality_badge_system,
                super::replay_shorts::hook_text_system,
            )
                .chain()
                .run_if(in_state(GameState::InGame))
                .run_if(resource_equals(GameMode::PgnReplay)),
        );

        // Debug system - toggle with F12 key
        app.add_systems(
            Update,
            debug_current_gamestate.run_if(input_toggle_active(false, KeyCode::F12)),
        );

        // Fullscreen toggle - F11 key
        app.add_systems(
            Update,
            toggle_fullscreen
                .run_if(input_just_pressed(KeyCode::F11))
                .before(crate::core::schedule::NetworkSet::Receive),
        );

        app.add_systems(Startup, setup_global_scene);

        // Add mesh picking plugin for 3D picking support (required in Bevy 0.18)
        app.add_plugins(MeshPickingPlugin);

        // Reset the persistent camera only when leaving the gameplay scene;
        // InGame/GameOver transitions keep the board visible behind the popup.
        app.add_systems(OnExit(InGameplay), (reset_game_camera,));

        // Replay cleanup
        app.add_systems(
            OnExit(GameState::InGame),
            super::replay::cleanup_replay.run_if(resource_equals(GameMode::PgnReplay)),
        );
    }
}

/// Run condition: the board is not in the TempleOS view, which has no game logic.
fn not_templeos(view_mode: Res<super::view_mode::ViewMode>) -> bool {
    !view_mode.is_templeos()
}
