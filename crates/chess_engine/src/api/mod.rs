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

mod game;
mod moves;
mod state;

pub use game::{new_game, reset_game};
pub use moves::{do_move, is_legal_move};
pub use state::{get_game_state, reply};
