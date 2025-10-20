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
//! accessed via system parameters:
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
//! # Reference
//!
//! Resource patterns from:
//! - `reference/bevy/examples/ecs/resources.rs` - Bevy resource basics
//! - `reference/bevy-3d-chess/src/game_state.rs` - Chess game state management
//! - `reference/chess_engine/src/types.rs` - Chess data structures

pub mod turn;
pub mod selection;
pub mod history;
pub mod timer;
pub mod captured;
pub mod game_over;
pub mod debug;
pub mod fast_board;
pub mod turn_state;

#[cfg(test)]
mod tests;

// Re-export all resources for convenience
pub use turn::*;
pub use selection::*;
pub use history::*;
pub use timer::*;
pub use captured::*;
pub use game_over::*;
pub use debug::*;
pub use fast_board::*;
pub use turn_state::*;
