//! Game history and state tracking resources
//!
//! Manages move history, captured pieces, and game over state.

pub mod captured;
pub mod game_over;
pub mod history;

// Re-export all public items
pub use captured::*;
pub use game_over::*;
pub use history::*;
