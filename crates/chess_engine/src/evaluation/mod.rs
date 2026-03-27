//! Position evaluation with piece-square tables
//!
//! Evaluates chess positions using:
//! - Material count (piece values)
//! - Positional bonuses (piece-square tables)
//! - Mobility (number of legal moves)
//! - King safety
//!
//! ## Module Organization
//!
//! - `pst` - Piece-square tables for positional evaluation
//! - `material` - Material balance evaluation
//! - `position` - Full position evaluation (material + positional + mobility)

mod material;
mod position;
mod pst;

pub use position::evaluate_position;
