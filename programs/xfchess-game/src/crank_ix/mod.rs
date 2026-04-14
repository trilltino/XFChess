//! Crank (scheduled task) instructions for automated game management.
//! 
//! Cranks enable automatic, recurring transactions on Ephemeral Rollups
//! without external infrastructure. Used here for:
//! - Time control enforcement (auto-flag timeout)
//! - Game inactivity cleanup

pub mod schedule_time_check;
pub mod crank_time_check;

pub use schedule_time_check::*;
pub use crank_time_check::*;
