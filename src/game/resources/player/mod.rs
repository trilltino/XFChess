//! Player interaction resources
//!
//! Manages player information and piece selection state.

pub mod player;
pub mod selection;

// Re-export all public items
pub use player::*;
pub use selection::*;
