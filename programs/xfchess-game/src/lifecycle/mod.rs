//! Plain Rust lifecycle logic shared by Anchor instruction adapters.

pub mod clock;
pub mod guards;
pub mod settlement;
pub mod terminal;
pub mod transitions;

pub use crate::state::GamePhase;
