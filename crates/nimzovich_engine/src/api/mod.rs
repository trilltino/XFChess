//! Public API for the chess engine
//!
//! Provides high-level functions for game management and AI move generation.
//! All functions include proper error handling and validation.
//!
//! ## Module Organization
//!
//! - `game` - Game lifecycle (new_game, reset_game)
//! - `moves` - Move execution and validation (do_move, is_legal_move)
//! - `state` - Game state queries and AI (get_game_state, reply)

pub mod game;
pub mod moves;
pub mod state;

#[cfg(feature = "std")]
pub use game::{game_from_fen, game_to_fen, new_game, reset_game, set_tt_size_mb};
pub use moves::{do_move, do_move_with_promo, is_legal_move};
pub use state::get_game_state;
#[cfg(feature = "search")]
pub use state::reply;
