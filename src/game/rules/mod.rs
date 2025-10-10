//! Chess rules module
//!
//! Pure logic for chess rules - no ECS dependencies.
//! Contains move validation and board state representation.

pub mod piece_moves;
pub mod board_state;

// Re-export commonly used items
pub use piece_moves::get_possible_moves;
pub use board_state::BoardState;
