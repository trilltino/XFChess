//! Alpha-beta search with iterative deepening
//!
//! This module implements the core AI search using:
//! - Negamax variant of alpha-beta pruning (ITERATIVE - no recursion)
//! - Iterative deepening for time management
//! - Transposition table for caching
//! - Move ordering for better pruning
//!
//! **Stack Overflow Fix**: Converted from recursive to iterative implementation
//! using explicit stack frames. This eliminates unbounded recursion and prevents
//! stack overflow at any search depth.
//!
//! ## Module Organization
//!
//! - `alphabeta` - Core alpha-beta search algorithm
//! - `quiescence` - Quiescence search to avoid horizon effect
//! - `ordering` - Move ordering heuristics
//! - `make_unmake` - Move making/unmaking utilities
//! - `iterative` - Iterative deepening wrapper

mod alphabeta;
mod iterative;
mod make_unmake;
mod ordering;
mod quiescence;

pub use iterative::find_best_move;
