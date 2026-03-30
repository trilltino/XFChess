//! Visual effects module
//!
//! Manages move hints and last move highlighting effects.

pub mod last_move;
pub mod move_hints;

// Re-export all public items
pub use last_move::*;
pub use move_hints::*;
