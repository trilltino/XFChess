//! System organization using SystemSets
//!
//! Defines execution order for game systems using Bevy's SystemSet feature.
//! This prevents subtle timing bugs by making system dependencies explicit.
//!
//! # Execution Order
//!
//! Systems run in this order each frame:
//! 1. **Input** - Handle user input (clicks, key presses)
//! 2. **Validation** - Validate moves, check game rules
//! 3. **Execution** - Execute validated moves, update game state
//! 4. **Visual** - Update visual representation (piece positions, highlights)
//!
//! # Benefits
//!
//! - **Predictable execution**: No race conditions between systems
//! - **Clear dependencies**: Easy to reason about data flow
//! - **Maintainability**: Adding new systems is straightforward
//!
//! # Reference
//!
//! - `reference/bevy/examples/ecs/system_sets.rs` - SystemSet patterns

use bevy::prelude::*;

/// System execution order for game logic
///
/// Each set runs in the order defined here, ensuring proper data flow
/// from input → validation → execution → visual updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum GameSystems {
    /// Input handling (pointer events, keyboard)
    ///
    /// Systems: piece selection, camera control
    Input,

    /// Move validation and game rule checks
    ///
    /// Systems: legal move generation, check detection
    Validation,

    /// Game state execution
    ///
    /// Systems: move execution, turn switching, timer updates, AI computation
    Execution,

    /// Visual updates
    ///
    /// Systems: piece transforms, highlights, move markers
    Visual,
}
