//! Input module - Bevy 0.17 picking and observer patterns
//!
//! Demonstrates modern Bevy 0.17 input handling using the new observer system and
//! picking backend, replacing the older Trigger<E> pattern.
//!
//! # Bevy 0.17 Migration
//!
//! **Updated Patterns**:
//! - ❌ `Trigger<Pointer<Click>>` (Bevy 0.16)
//! - ✅ `On<Pointer<Click>>` (Bevy 0.17)
//! - ❌ `EventReader<Pointer<Click>>` (Bevy 0.16)
//! - ✅ `MessageReader<Pointer<Click>>` (Bevy 0.17)
//!
//! # Architecture
//!
//! - `pointer` - Observer-based pointer event handlers and helpers
//! - Observer functions for material updates (hover effects)
//! - Timer resource for input debouncing
//!
//! # Reference
//!
//! Implementation follows:
//! - `reference/bevy/examples/picking/observers.rs` - Observer pattern examples
//! - `reference/bevy/examples/input/mouse_input.rs` - Input handling
//! - `reference/bevy-inspector-egui/` - Integration with UI systems
//!
//! The observer pattern allows decoupled event handling where entities can register
//! callbacks via `.observe(callback)` instead of global event polling.

pub mod pointer;

// Re-export commonly used items
pub use pointer::*;
