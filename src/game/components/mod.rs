//! Chess game components module
//!
//! Components are pure data structures with no logic.
//! Organized by domain: pieces, moves, and game state.

pub mod piece;
pub mod game_state;

#[cfg(test)]
mod tests;

// Re-export all components for convenience
pub use game_state::*;
pub use piece::*;
