//! System parameter groups for game resources
//!
//! Provides convenient SystemParam types that group related resources together,
//! following the bevy_egui pattern of using SystemParams for cleaner APIs.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::{
    CapturedPieces, CurrentGamePhase, CurrentTurn, GameOverState,
};
use crate::game::resources::player::selection::Selection;
use crate::engine::board_state::ChessEngine;

/// System parameter grouping game state resources
///
/// Provides convenient access to all game state resources in a single parameter.
/// This follows the bevy_egui pattern of using SystemParams for cleaner APIs.
///
/// # Resources Included
///
/// - [`CurrentTurn`] - Whose turn it is
/// - [`CurrentGamePhase`] - Current game phase (Playing, Check, etc.)
/// - [`GameOverState`] - Game end conditions
/// - [`CapturedPieces`] - Material tracking
#[derive(SystemParam)]
pub struct GameStateParams<'w> {
    /// Current turn information
    #[allow(dead_code)]
    pub current_turn: Res<'w, CurrentTurn>,
    /// Current game phase
    pub game_phase: Res<'w, CurrentGamePhase>,
    /// Game over state
    pub game_over: Res<'w, GameOverState>,
    /// Captured pieces tracking
    pub captured: Res<'w, CapturedPieces>,
    /// Piece selection state
    #[allow(dead_code)]
    pub selection: ResMut<'w, Selection>,
    /// Chess engine for move validation
    #[allow(dead_code)]
    pub engine: ResMut<'w, ChessEngine>,
}

/// System parameter grouping AI-related resources
///
/// Provides convenient access to AI configuration and state.
///
/// # Resources Included
///
/// - [`ChessAIResource`] - AI configuration (mode, difficulty)
/// - [`PendingAIMove`] - Optional pending AI move computation
/// - [`AIStatistics`] - AI performance statistics
///
/// # Example
///
/// ```rust,ignore
/// use crate::game::resources::AIParams;
///
/// fn check_ai_thinking(ai_params: AIParams) {
///     if ai_params.pending_ai.is_some() {
///         println!("AI is thinking...");
///     }
/// }
/// ```
#[derive(SystemParam)]
pub struct AIParams<'w> {
    /// AI configuration
    #[allow(dead_code)]
    pub ai_config: Res<'w, crate::game::ai::ChessAIResource>,
    /// Pending AI move computation
    #[allow(dead_code)]
    pub pending_ai: Option<Res<'w, crate::game::ai::PendingAIMove>>,
    /// AI statistics
    #[allow(dead_code)]
    pub ai_stats: Res<'w, crate::game::ai::AIStatistics>,
}
