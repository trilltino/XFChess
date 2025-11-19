//! System parameter groups for game resources
//!
//! Provides convenient SystemParam types that group related resources together,
//! following the bevy_egui pattern of using SystemParams for cleaner APIs.
//!
//! # Usage
//!
//! Instead of:
//! ```rust,ignore
//! fn my_system(
//!     current_turn: Res<CurrentTurn>,
//!     game_phase: Res<CurrentGamePhase>,
//!     game_over: Res<GameOverState>,
//!     captured: Res<CapturedPieces>,
//! ) {
//!     // ...
//! }
//! ```
//!
//! Use:
//! ```rust,ignore
//! fn my_system(game_state: GameStateParams) {
//!     // Access via game_state.current_turn, game_state.game_phase, etc.
//! }
//! ```

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::{
    CapturedPieces, ChessEngine, CurrentGamePhase, CurrentTurn, FastBoardState,
    GameOverState, GameTimer, MoveHistory, Players, Selection, TurnStateContext,
};

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
///
/// # Example
///
/// ```rust,ignore
/// fn check_game_state(game_state: GameStateParams) {
///     if game_state.game_over.is_game_over() {
///         return; // Don't process moves when game is over
///     }
///
///     if game_state.current_turn.color == PieceColor::White {
///         // White's turn logic
///     }
/// }
/// ```
/// System parameter grouping game state resources
///
/// Reserved for future use - provides convenient access to game state resources.
#[derive(SystemParam)]
#[allow(dead_code)] // Public API - reserved for future system implementations
pub struct GameStateParams<'w> {
    /// Current turn information
    pub current_turn: Res<'w, CurrentTurn>,
    /// Current game phase
    pub game_phase: Res<'w, CurrentGamePhase>,
    /// Game over state
    pub game_over: Res<'w, GameOverState>,
    /// Captured pieces tracking
    pub captured: Res<'w, CapturedPieces>,
}

/// System parameter grouping game history and timing resources
///
/// Provides access to move history and game timer resources.
///
/// # Resources Included
///
/// - [`MoveHistory`] - Complete move record
/// - [`GameTimer`] - Time control
///
/// # Example
///
/// ```rust,ignore
/// fn display_game_info(history: GameHistoryParams) {
///     println!("Moves: {}", history.move_history.len());
///     println!("White time: {:.1}s", history.game_timer.white_time_left);
/// }
/// ```
/// System parameter grouping game history and timing resources
///
/// Reserved for future use - provides convenient access to history and timer resources.
#[derive(SystemParam)]
#[allow(dead_code)] // Public API - reserved for future system implementations
pub struct GameHistoryParams<'w> {
    /// Move history
    pub move_history: Res<'w, MoveHistory>,
    /// Game timer
    pub game_timer: Res<'w, GameTimer>,
}

/// System parameter grouping player interaction resources
///
/// Provides access to selection and board state resources.
///
/// # Resources Included
///
/// - [`Selection`] - Currently selected piece
/// - [`FastBoardState`] - Bitboard representation
///
/// # Example
///
/// ```rust,ignore
/// fn highlight_selection(interaction: PlayerInteractionParams) {
///     if let Some((x, y)) = interaction.selection.selected_position {
///         // Highlight selected square
///     }
/// }
/// ```
/// System parameter grouping player interaction resources
///
/// Reserved for future use - provides convenient access to selection and board state.
#[derive(SystemParam)]
#[allow(dead_code)] // Public API - reserved for future system implementations
pub struct PlayerInteractionParams<'w> {
    /// Current selection
    pub selection: Res<'w, Selection>,
    /// Fast board state
    pub fast_board: Res<'w, FastBoardState>,
}

/// System parameter grouping turn management resources
///
/// Provides access to turn and turn state resources.
///
/// # Resources Included
///
/// - [`CurrentTurn`] - Current turn
/// - [`TurnStateContext`] - Fine-grained turn phase
///
/// # Example
///
/// ```rust,ignore
/// fn process_turn(turn: TurnParams) {
///     if turn.turn_context.phase.accepts_input() {
///         // Process player input
///     }
/// }
/// ```
/// System parameter grouping turn management resources
///
/// Reserved for future use - provides convenient access to turn resources.
#[derive(SystemParam)]
#[allow(dead_code)] // Public API - reserved for future system implementations
pub struct TurnParams<'w> {
    /// Current turn
    pub current_turn: Res<'w, CurrentTurn>,
    /// Turn state context
    pub turn_context: Res<'w, TurnStateContext>,
}

/// System parameter grouping engine and player resources
///
/// Provides access to chess engine and player information.
///
/// # Resources Included
///
/// - [`ChessEngine`] - Chess engine state
/// - [`Players`] - Player information
///
/// # Example
///
/// ```rust,ignore
/// fn validate_move(engine_params: EngineParams, from: (u8, u8), to: (u8, u8)) -> bool {
///     let color = engine_params.players.current(engine_params.current_turn.color).color;
///     // Use engine to validate move
/// }
/// ```
/// System parameter grouping engine and player resources
///
/// Reserved for future use - provides convenient access to engine and player resources.
#[derive(SystemParam)]
#[allow(dead_code)] // Public API - reserved for future system implementations
pub struct EngineParams<'w> {
    /// Chess engine
    pub engine: Res<'w, ChessEngine>,
    /// Players
    pub players: Res<'w, Players>,
    /// Current turn (needed for engine operations)
    pub current_turn: Res<'w, CurrentTurn>,
}

/// System parameter grouping all game resources
///
/// Provides access to all game resources in a single parameter.
/// Use this when you need access to multiple resource groups.
///
/// # Example
///
/// ```rust,ignore
/// fn comprehensive_system(all: AllGameParams) {
///     // Access all resources via all.game_state, all.history, etc.
/// }
/// ```
/// System parameter grouping all game resources
///
/// Reserved for future use - provides convenient access to all game resources.
#[derive(SystemParam)]
#[allow(dead_code)] // Public API - reserved for future system implementations
pub struct AllGameParams<'w> {
    /// Game state resources
    pub game_state: GameStateParams<'w>,
    /// History and timing
    pub history: GameHistoryParams<'w>,
    /// Player interaction
    pub interaction: PlayerInteractionParams<'w>,
    /// Turn management
    pub turn: TurnParams<'w>,
    /// Engine and players
    pub engine: EngineParams<'w>,
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
/// System parameter grouping AI-related resources
///
/// Reserved for future use - provides convenient access to AI resources.
#[derive(SystemParam)]
#[allow(dead_code)] // Public API - reserved for future system implementations
pub struct AIParams<'w> {
    /// AI configuration
    pub ai_config: Res<'w, crate::game::ai::ChessAIResource>,
    /// Pending AI move computation
    pub pending_ai: Option<Res<'w, crate::game::ai::PendingAIMove>>,
    /// AI statistics
    pub ai_stats: Res<'w, crate::game::ai::AIStatistics>,
}
