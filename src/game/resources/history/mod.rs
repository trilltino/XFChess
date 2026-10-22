//! What has happened in the game: the move list, captured pieces, and the result.

pub mod captured;
pub mod game_over;
pub mod moves;

// Re-export all public items
pub use captured::*;
pub use game_over::*;
pub use moves::*;
