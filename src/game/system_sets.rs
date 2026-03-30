//! System organization using SystemSets
//!
//! Defines execution order for game systems using Bevy's SystemSet feature.
//! This prevents subtle timing bugs by making system dependencies explicit.
//!
//! # Execution Order
//!
//! Systems run in this order each frame (when `GameState::InGame` is active):
//!
//! 1. **Input** (`GameSystems::Input`)
//!    - Handle user input (clicks, key presses)
//!    - Camera movement and rotation
//!    - Piece selection via observers
//!
//! 2. **Validation** (`GameSystems::Validation`)
//!    - Synchronize FastBoardState bitboards
//!    - Validate moves, check game rules
//!    - Prepare data for execution systems
//!
//! 3. **Execution** (`GameSystems::Execution`)
//!    - Execute validated moves
//!    - Update game state (phase, timer, turn)
//!    - Detect check, checkmate, stalemate
//!    - AI move computation (if applicable)
//!
//! 4. **Visual** (`GameSystems::Visual`)
//!    - Update visual representation
//!    - Highlight possible moves
//!    - Animate piece movement
//!
//! # System Set Configuration
//!
//! System sets are configured in [`crate::game::plugin::GamePlugin`] using:
//!
//! ```rust,ignore
//! app.configure_sets(
//!     Update,
//!     (
//!         GameSystems::Input,
//!         GameSystems::Validation,
//!         GameSystems::Execution,
//!         GameSystems::Visual,
//!     )
//!         .chain()
//!         .run_if(in_state(GameState::InGame)),
//! );
//! ```
//!
//! The `.chain()` ensures sequential execution within each set, and
//! `.run_if(in_state(GameState::InGame))` ensures systems only run during gameplay.
//!
//! # Benefits
//!
//! - **Predictable execution**: No race conditions between systems
//! - **Clear dependencies**: Easy to reason about data flow
//! - **Maintainability**: Adding new systems is straightforward
//! - **Performance**: Systems can run in parallel within sets when possible
//!
//! # Adding New Systems
//!
//! When adding a new system, determine which set it belongs to:
//!
//! - **Input**: User interaction, input processing
//! - **Validation**: Data synchronization, move validation
//! - **Execution**: Game state updates, turn management
//! - **Visual**: Rendering updates, animations
//!
//! Then add it to the appropriate set:
//!
//! ```rust,ignore
//! app.add_systems(
//!     Update,
//!     my_new_system.in_set(GameSystems::Validation),
//! );
//! ```
//!
//! # Reference
//!
//! - `reference/bevy/examples/ecs/system_sets.rs` - SystemSet patterns
//! - `reference/bevy/crates/bevy_ecs/src/schedule/system_set.rs` - SystemSet API

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
