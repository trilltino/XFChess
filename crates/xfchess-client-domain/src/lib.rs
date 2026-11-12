//! Pure client domain types. This crate has no renderer, ECS, transport, or OS dependency.

pub mod time_control;

pub use time_control::{TimeCategory, TimeControl};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientGameMode {
    SinglePlayer,
    LocalMultiplayer,
    Competitive,
    Online,
    Spectator,
    Replay,
    Puzzle,
}
