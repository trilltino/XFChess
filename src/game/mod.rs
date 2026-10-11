//! Chess gameplay: turn flow, move execution, clocks, AI, replays and board sync.
//! [`GamePlugin`] registers it all in the ordered
//! [`GameSystems`](crate::core::schedule::GameSystems) sets. See [README.md](README.md).

pub mod ai;
pub mod board_coords;
pub mod camera_modes;
pub mod components;
pub mod events;
pub mod plugin;
pub mod replay;
pub mod replay_braid;
pub mod replay_shorts;
pub mod resources;
pub mod shorts_state;
pub mod sync;
pub mod systems;
pub mod time_control;
pub mod types;
pub mod view_mode;

pub use plugin::GamePlugin;
