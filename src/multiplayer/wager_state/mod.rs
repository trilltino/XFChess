//! Wager State Integration
//!
//! This module handles wager information from the web UI via CLI arguments
//! and displays it in the game UI.

pub mod state;
pub mod ui;
pub mod plugin;

pub use plugin::WagerPlugin;
pub use state::WagerState;
