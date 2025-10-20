//! Core module - Fundamental game state management
//!
//! Provides the foundational state machine for the XFChess application using Bevy 0.17's
//! state system. This module demonstrates idiomatic Bevy state management patterns.
//!
//! # State Architecture
//!
//! - `GameState` - Primary state enum (LaunchMenu, Multiplayer)
//! - `LaunchMenu` - Computed state for conditional system execution
//!
//! # Bevy 0.17 Patterns Used
//!
//! - `States` trait for primary state management
//! - `ComputedStates` for derived state logic
//! - State-based system scheduling with `in_state()` and `OnEnter()`
//!
//! # Reference
//!
//! Follows patterns from `reference/bevy/examples/ecs/state.rs` for proper state transitions
//! and computed state implementation.

pub mod state;

// Re-export commonly used items
pub use state::*;
