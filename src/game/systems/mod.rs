//! Chess game systems module - ECS logic implementation
//!
//! Systems are functions that run each frame (or on events) to implement game behavior.
//! This module organizes all chess game logic into focused, testable systems following
//! Bevy 0.17's idiomatic ECS patterns.
//!
//! # System Organization
//!
//! ## Core Gameplay Systems
//! - [`input`] - Observer-based piece selection and square click handling
//! - [`game_logic`] - Check/checkmate/stalemate detection and game phase updates
//! - [`visual`] - Square highlighting, piece animations, and visual feedback
//! - [`board_sync`] - Synchronizes FastBoardState bitboards with ECS entities
//!
//! ## User Interface Systems
//! - [`camera`] - RTS-style WASD camera controls for board observation
//!
//! # System Execution Order
//!
//! Systems are organized into SystemSets for predictable execution order:
//!
//! ```text
//! ┌─────────────────┐
//! │ Input           │  User clicks pieces/squares
//! │ (Observers)     │  → Updates Selection resource
//! └────────┬────────┘
//!          ↓
//! ┌─────────────────┐
//! │ BoardSync       │  Rebuilds FastBoardState from ECS
//! │                 │  → Enables O(1) move validation
//! └────────┬────────┘
//!          ↓
//! ┌─────────────────┐
//! │ GameLogic       │  Validates moves, detects check/mate
//! │                 │  → Updates GamePhase, GameOverState
//! └────────┬────────┘
//!          ↓
//! ┌─────────────────┐
//! │ Visual          │  Updates highlights and animations
//! │                 │  → Provides player feedback
//! └────────┬────────┘
//!          ↓
//! ┌─────────────────┐
//! │ Camera          │  Processes WASD movement input
//! │ (Parallel)      │  → Smooth camera panning
//! └─────────────────┘
//! ```
//!
//! # Bevy 0.17 System Architecture
//!
//! **System Signatures**:
//! Systems declare their data dependencies through parameters:
//!
//! ```rust,ignore
//! fn my_system(
//!     query: Query<(&Component1, &mut Component2)>,  // Read Component1, modify Component2
//!     resource: Res<MyResource>,                     // Read-only resource access
//!     mut commands: Commands,                        // Spawn/despawn entities
//! ) {
//!     // System implementation
//! }
//! ```
//!
//! **Scheduling**:
//! Systems are registered in [`crate::game::plugin::GamePlugin`]:
//!
//! ```rust,ignore
//! app.add_systems(Update, (
//!     sync_fast_board_state,
//!     update_game_phase,
//!     highlight_possible_moves,
//! ).chain()); // Chain ensures sequential execution
//! ```
//!
//! # Observer Pattern (Bevy 0.17)
//!
//! Input handling uses Bevy's observer pattern for entity-specific event handling:
//!
//! ```rust,ignore
//! // Attach observers to entities during spawning
//! commands.spawn(PieceMesh)
//!     .observe(on_piece_click); // Runs when THIS piece is clicked
//!
//! commands.spawn(SquareMesh)
//!     .observe(on_square_click); // Runs when THIS square is clicked
//! ```
//!
//! This replaces the older polling pattern (`EventReader`) with direct event routing
//! to specific entities, improving both performance and code clarity.
//!
//! # State-Conditional Execution
//!
//! Most systems only run during active gameplay:
//!
//! ```rust,ignore
//! .add_systems(Update, my_system.run_if(in_state(GameState::Multiplayer)))
//! ```
//!
//! This prevents gameplay systems from running in menus or other non-game states.
//!
//! # Performance Considerations
//!
//! - **Query Iteration**: Systems iterate over entities matching their query filters
//! - **Parallel Execution**: Systems without shared mutable resources run in parallel
//! - **Change Detection**: Use `Changed<T>` filters to only process modified data
//! - **FastBoardState**: Bitboard cache provides O(1) position lookups vs O(n) iteration
//!
//! # Testing Strategy
//!
//! Systems are integration-tested using Bevy's App test harness:
//!
//! ```rust,ignore
//! #[test]
//! fn test_move_execution() {
//!     let mut app = App::new();
//!     app.add_systems(Update, execute_move_system);
//!     app.init_resource::<Selection>();
//!
//!     // Simulate piece movement
//!     app.world_mut().resource_mut::<Selection>()
//!         .selected_entity = Some(piece_entity);
//!
//!     app.update(); // Run one frame
//!
//!     // Verify piece moved
//!     let piece = app.world().get::<Piece>(piece_entity).unwrap();
//!     assert_eq!((piece.x, piece.y), expected_position);
//! }
//! ```
//!
//! See `tests/systems_integration_tests.rs` for full system testing examples.
//!
//! # Reference
//!
//! System design patterns from:
//! - `reference/bevy/examples/ecs/ecs_guide.rs` - Comprehensive system examples
//! - `reference/bevy/examples/ecs/parallel_query.rs` - Performance optimization
//! - `reference/bevy/examples/picking/mesh_picking.rs` - Observer pattern usage
//! - `reference/bevy-3d-chess/src/systems/` - Alternative chess ECS implementation

pub mod input;
pub mod visual;
pub mod game_logic;
pub mod camera;
pub mod board_sync;

// Re-export all public systems for convenience
pub use input::*;
pub use visual::*;
pub use game_logic::*;
pub use camera::*;
pub use board_sync::*;
