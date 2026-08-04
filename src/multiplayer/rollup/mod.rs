//! MagicBlock Ephemeral Rollup client integration: delegation lifecycle,
//! sub-second move submission, and session-key management for the ER path.

pub mod bridge;
pub mod magicblock;
pub mod manager;
pub mod mvp_plugin;
pub mod session_keys;

pub use magicblock::*;
pub use manager::*;
