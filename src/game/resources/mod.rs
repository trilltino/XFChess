//! Chess game resources module
//!
//! Resources are global singletons that can be accessed by any system.
//! Organized by domain: turn state, selection, history, and timing.

pub mod turn;
pub mod selection;
pub mod history;
pub mod timer;

// Re-export all resources for convenience
pub use turn::*;
pub use selection::*;
pub use history::*;
pub use timer::*;
