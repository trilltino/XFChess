//! Chess game logic module - Complete chess implementation with ECS
//!
//! Implements a fully functional chess game using Bevy 0.17's ECS architecture, with
//! clean separation between pure game logic and ECS systems. Designed for potential
//! integration with the modularized chess engine in `reference/chess_engine/`.
//!
//! # Module Organization
//!
//! - `components` - ECS components (Piece, HasMoved, GamePhase, MoveRecord)
//! - `resources` - Global game state (CurrentTurn, GameTimer, Selection, MoveHistory)
//! - `systems` - ECS systems for gameplay (input, movement, visual, game_logic)
//! - `rules` - Pure chess logic (move validation, board state)
//! - `plugin` - GamePlugin that registers everything with reflection support
//!
//! # ECS Architecture
//!
//! **Data-Oriented Design**:
//! - Components hold data (piece type, position, movement state)
//! - Resources track global state (whose turn, timer, selection)
//! - Systems implement behavior (handle clicks, validate moves, update visuals)
//!
//! **System Ordering**:
//! 1. Input systems (`handle_piece_selection`)
//! 2. Move execution (`move_piece`)
//! 3. Game state updates (`update_game_phase`, `update_game_timer`)
//! 4. Visual updates (`highlight_possible_moves`, `animate_piece_movement`)
//!
//! # Reference Integration
//!
//! - `reference/chess_engine/` - Potential AI opponent with alpha-beta search
//! - `reference/bevy-3d-chess/` - Alternative ECS chess implementation
//! - `reference/bevy/examples/ecs/` - ECS pattern examples
//!
//! # Future Enhancements
//!
//! Leveraging `reference/chess_engine/`:
//! - AI opponent using minimax/alpha-beta pruning (1800-2000 ELO)
//! - Position evaluation with material + positional factors
//! - Transposition tables for move caching
//! - Opening book integration

pub mod components;
pub mod resources;
pub mod systems;
pub mod rules;
pub mod plugin;
pub mod ai;
pub mod system_sets;

// Re-export the plugin and camera controller (main entry points)
pub use plugin::GamePlugin;
pub use systems::CameraController;
