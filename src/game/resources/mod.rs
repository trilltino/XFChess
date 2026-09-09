// Submodules
pub mod active_time_control;
pub mod history;
pub mod menu_sounds;
pub mod player;
pub mod sounds;
pub mod turn;

// Root-level modules
pub mod debug;
pub mod first_move_deadline;
pub mod system_params;

#[cfg(test)]
mod tests;

// Re-export all resources for convenience
pub use active_time_control::*;
pub use debug::*;
pub use first_move_deadline::*;
pub use history::*;
pub use menu_sounds::*;
pub use player::*;
pub use sounds::*;
pub use system_params::*;
pub use turn::*;
