//! Chess game plugin
//!
//! This plugin registers all game systems and resources.
//! Systems are organized with run conditions to optimize performance.

use bevy::prelude::*;
use bevy::input::common_conditions::input_toggle_active;
use bevy::ecs::system::SystemParam;
use crate::core::{GameState, debug_current_gamestate};
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::game::components::{GamePhase, HasMoved, MoveRecord, SelectedPiece};
use super::resources::*;
use super::systems::*;
use super::system_sets::GameSystems;
use super::ai::{AIPlugin, ChessAIResource, PendingAIMove, AIStatistics};
use bevy_egui::EguiContexts;

pub struct GamePlugin;

/// System parameter grouping game state resources
#[derive(SystemParam)]
pub struct GameStateParams<'w> {
    pub captured: Res<'w, CapturedPieces>,
    pub current_turn: Res<'w, CurrentTurn>,
    pub game_phase: Res<'w, CurrentGamePhase>,
    pub game_over: Res<'w, GameOverState>,
}

/// System parameter grouping AI-related resources
#[derive(SystemParam)]
pub struct AIParams<'w> {
    pub ai_config: Res<'w, ChessAIResource>,
    pub pending_ai: Option<Res<'w, PendingAIMove>>,
    pub ai_stats: Res<'w, AIStatistics>,
}

/// Wrapper function for game_status_ui that handles errors
fn game_status_ui_wrapper(
    contexts: EguiContexts,
    game_state: GameStateParams,
    ai_params: AIParams,
    next_state: ResMut<NextState<GameState>>,
) {
    // Silently ignore NoEntities errors - this happens during state transitions
    // when the egui context isn't available yet
    let _ = crate::ui::game_status_ui(
        contexts,
        game_state,
        ai_params,
        next_state,
    );
}

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
            .init_resource::<FastBoardState>()
            .init_resource::<TurnStateContext>();

        // Register types for reflection (needed for inspector)
        app.register_type::<CurrentTurn>()
            .register_type::<CurrentGamePhase>()
            .register_type::<GameTimer>()
            .register_type::<MoveHistory>()
            .register_type::<CapturedPieces>()
            .register_type::<GameOverState>()
            .register_type::<FastBoardState>()
            .register_type::<TurnStateContext>()
            .register_type::<TurnPhase>()
            .register_type::<GamePhase>()
            .register_type::<MoveRecord>()
            .register_type::<Piece>()
            .register_type::<PieceColor>()
            .register_type::<PieceType>()
            .register_type::<HasMoved>()
            .register_type::<SelectedPiece>()
            .register_type::<CameraController>();

        // Add AI plugin
        app.add_plugins(AIPlugin);

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
                .run_if(in_state(GameState::Multiplayer)),
        );

        // Register systems with run conditions
        // Systems are assigned to sets for predictable execution order
        // NOTE: Input handling is now done via observers on entities (.observe())
        // so we don't need handle_piece_selection/clear_selection_on_empty_click systems
        app.add_systems(
            Update,
            (
                // Input set: Handle user input
                camera_movement_system.in_set(GameSystems::Input),
                camera_zoom_input_system.in_set(GameSystems::Input),
                camera_zoom_system.in_set(GameSystems::Input),

                // Validation set: Sync board state before validation
                sync_fast_board_state.in_set(GameSystems::Validation),

                // Execution set: Update game state
                update_game_phase.in_set(GameSystems::Execution),
                update_game_timer.in_set(GameSystems::Execution),

                // Visual set: Update rendering
                highlight_possible_moves.in_set(GameSystems::Visual),
                animate_piece_movement.in_set(GameSystems::Visual),
            ),
        );

        // Add UI system separately (it returns Result)
        app.add_systems(
            Update,
            game_status_ui_wrapper.run_if(in_state(GameState::Multiplayer)),
        );

        // Debug system - toggle with F12 key
        app.add_systems(
            Update,
            debug_current_gamestate.run_if(input_toggle_active(true, KeyCode::F12)),
        );
    }
}
