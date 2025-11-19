//! Core module - Fundamental game state management and application infrastructure
//!
//! Provides the foundational state machine and core application setup for the XFChess
//! application using Bevy 0.17's state system. This module demonstrates idiomatic Bevy
//! state management patterns and plugin architecture.
//!
//! # Architecture Overview
//!
//! ## State Architecture
//!
//! - `GameState` - Primary state enum with 7 states (Splash → Loading → MainMenu → Settings → InGame → Paused → GameOver)
//! - `MenuState` - Sub-state for menu navigation within MainMenu
//! - `InMenus` - Computed state active during MainMenu and Settings
//! - `InGameplay` - Computed state active during InGame, Paused, and GameOver
//!
//! ## Core Plugin
//!
//! The [`CorePlugin`] sets up fundamental application infrastructure:
//! - Panic hook configuration for detailed crash reporting
//! - Window configuration via [`WindowConfig`] resource
//! - Core state management resources
//! - Settings persistence
//!
//! ## Resources
//!
//! - [`WindowConfig`] - Window settings configuration
//! - [`GameSettings`] - User preferences (graphics, audio, etc.)
//! - [`GameStatistics`] - Player performance tracking
//! - [`PreviousState`] - Navigation state tracking
//!
//! # Bevy 0.17 Patterns Used
//!
//! - `States` trait for primary state management
//! - `SubStates` for hierarchical state relationships
//! - `ComputedStates` for derived state logic
//! - State-based system scheduling with `in_state()` and `OnEnter()`
//! - Plugin architecture with `build()` and `finish()` methods
//! - Resource initialization patterns
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use bevy::prelude::*;
//! use xfchess::core::CorePlugin;
//!
//! App::new()
//!     .add_plugins(CorePlugin)
//!     .add_plugins(DefaultPlugins.set(WindowPlugin {
//!         primary_window: Some(window_config.to_window()),
//!         ..default()
//!     }))
//!     // ... other plugins
//! ```
//!
//! # Reference
//!
//! Follows patterns from:
//! - `reference/bevy/examples/games/game_menu.rs` - Multi-state game flow
//! - `reference/bevy/examples/state/states.rs` - Modern state system
//! - `reference/bevy/examples/state/sub_states.rs` - Hierarchical states
//! - `reference/bevy/crates/bevy_app/src/plugin.rs` - Plugin architecture

pub mod error;
pub mod error_handling;
pub mod plugin;
pub mod resources;
pub mod settings_persistence;
pub mod states;
pub mod window_config;

// Re-export commonly used items
pub use plugin::CorePlugin;
pub use resources::*;
pub use states::*;
pub use window_config::WindowConfig;
