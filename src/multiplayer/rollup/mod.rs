//! MagicBlock Ephemeral Rollup integration: delegate a game account, stream moves to
//! the rollup, then settle back to Solana.

pub mod bridge;
pub mod magicblock;
pub mod manager;
pub mod mvp_plugin;
pub mod session_keys;

pub use magicblock::*;
pub use manager::*;
