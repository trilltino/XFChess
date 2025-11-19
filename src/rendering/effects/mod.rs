//! Visual effects module
//!
//! Manages move hints, last move highlighting, and dynamic lighting effects.

pub mod dynamic_lighting;
pub mod last_move;
pub mod move_hints;

// Re-export all public items
pub use dynamic_lighting::*;
pub use last_move::*;
pub use move_hints::*;
