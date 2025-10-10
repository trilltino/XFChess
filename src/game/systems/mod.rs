//! Chess game systems module
//!
//! Systems contain the game logic that operates on components and resources.
//! Organized by responsibility: input handling, movement, visuals, and game logic.

pub mod input;
pub mod movement;
pub mod visual;
pub mod game_logic;

// Re-export all public systems for convenience
pub use input::*;
pub use movement::*;
pub use visual::*;
pub use game_logic::*;
