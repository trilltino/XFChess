//! Chess AI opponent system
//!
//! This module integrates the chess engine for AI opponent functionality.
//! The AI runs asynchronously to avoid blocking the main game thread.
//!
//! # Architecture
//!
//! - `ChessAIResource`: Main resource containing game mode and difficulty settings
//! - `PendingAIMove`: Task handle for async AI computation
//! - Systems spawn AI tasks when it's the AI's turn
//! - Systems poll completed tasks and execute the AI's chosen move
//!
//! # Integration Strategy
//!
//! The ECS board state is the source of truth. Before AI computation:
//! 1. Sync ECS pieces to internal engine board representation
//! 2. Spawn async task to compute best move
//! 3. Poll task completion
//! 4. Execute move through normal movement system
//!
//! This ensures AI moves follow the same validation rules as human moves.

use bevy::prelude::*;
use crate::rendering::pieces::PieceColor;

pub mod resource;
pub mod systems;

// Re-export for convenience
pub use resource::{ChessAIResource, GameMode, AIDifficulty};
pub use systems::{AIPlugin, PendingAIMove, AIStatistics};
