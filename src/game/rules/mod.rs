//! Chess rules module - Pure game logic without ECS coupling
//!
//! Implements chess move validation and board state management using pure functions,
//! allowing easy testing and potential integration with the chess engine in
//! `reference/chess_engine/`.
//!
//! # Architecture
//!
//! This module maintains a clean separation between game logic and ECS systems:
//! - **Pure functions** for move validation (easy to unit test)
//! - **Lightweight BoardState** for snapshot-based validation
//! - **No Component/Resource dependencies** for portability
//!
//! # Module Structure
//!
//! - `piece_moves` - Movement rules for each piece type (pawn, knight, bishop, rook, queen, king)
//! - `board_state` - Board representation for move validation queries
//!
//! # Reference
//!
//! Chess rules implementation references:
//! - `reference/chess_engine/move_gen.rs` - Advanced move generation algorithms
//! - `reference/bevy-3d-chess/` - Alternative ECS chess implementation
//!
//! Future integration could leverage the modularized chess engine for:
//! - AI opponent using alpha-beta pruning
//! - Advanced move validation (en passant, castling)
//! - Position evaluation and opening book

pub mod piece_moves;
pub mod board_state;

#[cfg(test)]
mod tests;

// Re-export commonly used items
pub use piece_moves::get_possible_moves;
pub use board_state::BoardState;
