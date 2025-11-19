//! System parameter groups for UI systems
//!
//! Provides convenient SystemParam types that group related resources together
//! for UI rendering, following the bevy_egui pattern of using SystemParams
//! for cleaner APIs.

use crate::core::{GameState, PreviousState};
use crate::game::resources::{AIParams, GameStateParams};
use crate::game::resources::{GameTimer, MoveHistory, Players};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::EguiContexts;

/// System parameter grouping UI-related resources
///
/// Provides convenient access to all resources needed for game UI rendering.
/// This follows the bevy_egui pattern of using SystemParams for cleaner APIs.
///
/// # Resources Included
///
/// - [`EguiContexts`] - Egui context for UI rendering
/// - [`GameStateParams`] - Game state resources
/// - [`AIParams`] - AI configuration and state
/// - [`MoveHistory`] - Move history for notation display
/// - [`GameTimer`] - Game timer for time display
/// - [`Players`] - Player information
/// - [`NextState<GameState>`] - State transitions
/// - [`PreviousState`] - Navigation state
///
/// # Example
///
/// ```rust,ignore
/// use crate::ui::system_params::GameUIParams;
///
/// fn my_ui_system(params: GameUIParams) {
///     let ctx = params.contexts.ctx_mut().unwrap();
///     // Use ctx to render UI
///     // Access game state via params.game_state
/// }
/// ```
/// System parameter grouping UI-related resources
///
/// Used by game_status_ui system (currently disabled but reserved for future use).
#[derive(SystemParam)]
#[allow(dead_code)] // Used by game_status_ui which is reserved for future use
pub struct GameUIParams<'w, 's> {
    /// Egui contexts for UI rendering
    pub contexts: EguiContexts<'w, 's>,
    /// Game state resources
    pub game_state: GameStateParams<'w>,
    /// AI parameters
    pub ai_params: AIParams<'w>,
    /// Move history
    pub move_history: Res<'w, MoveHistory>,
    /// Game timer
    pub game_timer: Res<'w, GameTimer>,
    /// Players
    pub players: Res<'w, Players>,
    /// Next state for transitions
    pub next_state: ResMut<'w, NextState<GameState>>,
    /// Previous state for navigation
    pub previous_state: ResMut<'w, PreviousState>,
}
