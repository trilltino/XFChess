//! UI module - Egui-based user interfaces
//!
//! Manages all UI rendering using `bevy_egui`, providing both gameplay UI
//! and development/debugging interfaces following Bevy 0.17 and bevy_egui best practices.
//!
//! # Module Organization
//!
//! - **styles**: Centralized theme system (colors, typography, components)
//! - **inspector**: Debug UI for inspecting ECS state (F1 to toggle)
//! - **game_ui**: Main game UI systems
//! - **system_params**: SystemParam groups for UI systems
//!
//! # Bevy Egui Integration
//!
//! Uses `bevy_egui` (0.37.1) which provides:
//! - `EguiContexts` system parameter for accessing egui context
//! - Automatic input handling and rendering
//! - Integration with Bevy's window and input systems
//! - Multi-pass rendering support for complex UIs
//!
//! # System Parameters
//!
//! UI systems use SystemParam groups for cleaner APIs:
//!
//! ```rust,ignore
//! use crate::ui::system_params::GameUIParams;
//!
//! fn my_ui_system(params: GameUIParams) {
//!     let ctx = params.contexts.ctx_mut()?;
//!     // Render UI using ctx
//! }
//! ```
//!
//! # Error Handling
//!
//! UI systems return `Result<(), QuerySingleError>` to handle cases where
//! the egui context may not be available (e.g., during state transitions).
//! Systems should gracefully handle these errors.
//!
//! # Reference
//!
//! Egui patterns follow:
//! - `reference/bevy_egui/src/lib.rs` - bevy_egui implementation patterns
//! - `reference/bevy-inspector-egui/` - Inspector UI implementation
//! - `bevy_egui` examples - Context access and layout patterns
//!
//! The inspector integration is particularly useful for debugging entity hierarchies,
//! component values, and resource state during development.

pub mod auth;
pub mod chat;
pub mod fps;
pub mod game_ui;
pub mod inspector;
pub mod promotion_ui;
pub mod styles;
pub mod system_params;
