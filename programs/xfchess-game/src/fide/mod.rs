//! Versioned fixed-offset Game extension. The legacy Borsh prefix is unchanged.
use anchor_lang::Space;

pub const LEGACY_GAME_SPACE: usize = 8 + crate::state::Game::INIT_SPACE;
pub const FIDE_HEADER_SIZE: usize = 66;
pub const FIDE_HISTORY_SIZE: usize = 151 * 34;
pub const FIDE_EXTENSION_SIZE: usize = FIDE_HEADER_SIZE + FIDE_HISTORY_SIZE;
pub const FIDE_GAME_SPACE: usize = LEGACY_GAME_SPACE + FIDE_EXTENSION_SIZE;
pub const GAME_ACCOUNT_SPACE: usize = if cfg!(feature = "move-validation") {
    FIDE_GAME_SPACE
} else { LEGACY_GAME_SPACE };

mod outcome;
pub use outcome::*;

#[cfg(feature = "move-validation")]
mod storage;
#[cfg(feature = "move-validation")]
mod actions;
#[cfg(feature = "move-validation")]
pub use actions::*;
#[cfg(feature = "move-validation")]
pub use storage::*;

#[cfg(all(test, feature = "move-validation"))]
mod tests;
