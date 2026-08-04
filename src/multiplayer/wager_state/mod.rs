//! Wager State Integration
//!
//! This module handles wager information from the web UI via CLI arguments
//! and displays it in the game UI.

pub mod plugin;
pub mod state;
pub mod ui;

pub use plugin::WagerPlugin;
