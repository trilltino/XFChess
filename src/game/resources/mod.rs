//! Chess game resources - Global game state management
//!
//! Resources are ECS singletons that provide shared mutable state across systems.
//! Unlike components (which are attached to entities), resources exist globally
//! and can be accessed by any system that needs them.
//!
//! # Resource Categories
//!
//! ## Turn Management
//! - [`CurrentTurn`] - Tracks whose turn it is and move numbers
//! - [`CurrentGamePhase`] - Wraps GamePhase as a resource
//! - [`TurnStateContext`] - Fine-grained turn flow state machine
//! - [`TurnPhase`] - Turn sub-states (WaitingForInput, AIThinking, etc.)
//!
//! ## Player Interaction
//! - [`Selection`] - Currently selected piece and valid moves
//!
//! ## Game History
//! - [`MoveHistory`] - Complete move record for undo/PGN export
//! - [`CapturedPieces`] - Material tracking and advantage calculation
//!
//! ## Game Timing
//! - [`GameTimer`] - Fischer increment time control
//!
//! ## Game Status
//! - [`GameOverState`] - Win/loss/draw conditions
//!
//! ## Performance
//! - [`FastBoardState`] - Bitboard representation for O(1) piece lookups
//!
//! ## Development
//! - [`DebugThrottle`] - Throttles debug logging to prevent spam
//!
//! # ECS Architecture: Components vs Resources
//!
//! **Use Components when:**
//! - Data is attached to specific entities (piece type, position)
//! - Multiple instances exist (many pieces, many squares)
//! - Data is queried per-entity
//!
//! **Use Resources when:**
//! - Data is global (whose turn, game timer)
//! - Only one instance exists (single selection, single history)
//! - Many systems need shared access
//!
//! # Integration
//!
//! All resources are registered in [`crate::game::plugin::GamePlugin`] and
//! accessed via system parameters. For convenience, use [`SystemParam`] groups
//! to access related resources together:
//!
//! ## Using SystemParam Groups (Recommended)
//!
//! ```rust,ignore
//! use crate::game::resources::GameStateParams;
//!
//! fn my_system(game_state: GameStateParams) {
//!     // Access multiple related resources via a single parameter
//!     if game_state.current_turn.color == PieceColor::White {
//!         game_state.game_over.is_game_over();
//!         game_state.captured.material_advantage();
//!     }
//! }
//! ```
//!
//! ## Direct Resource Access
//!
//! ```rust,ignore
//! fn my_system(
//!     current_turn: Res<CurrentTurn>,
//!     mut selection: ResMut<Selection>,
//!     history: Res<MoveHistory>,
//! ) {
//!     // Read current turn (immutable)
//!     if current_turn.color == PieceColor::White {
//!         // Modify selection (mutable)
//!         selection.clear();
//!     }
//!
//!     // Read history
//!     if let Some(last_move) = history.last_move() {
//!         println!("Last move: {:?}", last_move);
//!     }
//! }
//! ```
//!
//! ## Available SystemParam Groups
//!
//! - [`GameStateParams`] - Turn, phase, game over, captured pieces
//! - [`GameHistoryParams`] - Move history and timer
//! - [`PlayerInteractionParams`] - Selection and board state
//! - [`TurnParams`] - Turn and turn state context
//! - [`EngineParams`] - Chess engine and players
//! - [`AllGameParams`] - All game resources (use sparingly)
//!
//! # Reference
//!
//! Resource patterns from:
//! - `reference/bevy/examples/ecs/resources.rs` - Bevy resource basics
//! - `reference/bevy-3d-chess/src/game_state.rs` - Chess game state management
//! - `reference/chess_engine/src/types.rs` - Chess data structures

// Submodules
pub mod engine;
pub mod history;
pub mod player;
pub mod sounds;
pub mod turn;

// Root-level modules
pub mod debug;
pub mod system_params;

#[cfg(test)]
mod tests;

// Re-export all resources for convenience
pub use debug::*;
pub use engine::*;
pub use history::*;
pub use player::*;
pub use sounds::*;
pub use system_params::*;
pub use turn::*;
