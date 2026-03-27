//! Chess game resources - Global game state management.
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
pub use history::*;
pub use player::*;
pub use sounds::*;
pub use system_params::*;
pub use turn::*;
