//! Visual effects module
//!
//! Manages move hints and last move highlighting effects.

pub mod check_highlight;
pub mod dynamic_lighting;
pub mod last_move;
pub mod move_hints;
pub mod sky;

// Re-export all public items
pub use check_highlight::*;
pub use dynamic_lighting::DynamicLightingPlugin;
pub use last_move::{init_arrow_assets, update_last_move_highlight_system, ArrowAssets, LastMoveArrow3D, LastMoveHighlight};
pub use move_hints::*;
pub use sky::SkyPlugin;
