//! Gameplay systems: camera control, game setup, turn/clock/game-over logic,
//! promotion, network-move application, and visual feedback. Board input lives in
//! [`crate::input`].

pub mod camera;
pub mod debug_visuals;
pub mod first_move_timer;
pub mod game_init;
pub mod game_logic;
pub mod network_move;
#[cfg(debug_assertions)]
pub mod picking_debug;
pub mod promotion;
pub mod shared;
pub mod spectate_sync;
pub mod visual;

// Re-export all public systems for convenience

pub use camera::*;
pub use game_init::*;
pub use game_logic::*;
pub use promotion::*;
pub use visual::*;
