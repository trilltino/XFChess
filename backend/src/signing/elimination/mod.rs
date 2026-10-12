//! Single-elimination tournaments: the bracket and its rules ([`bracket`]), state
//! transitions on the stored record ([`service`]) and on-chain mirroring
//! ([`chain`]). Swiss events live in [`crate::signing::swiss`]; both are driven by
//! [`crate::signing::orchestrator`].

pub mod bracket;
pub mod chain;
pub mod service;

pub use bracket::{
    Decision, EliminationError, EliminationState, GameApplied, Stage, TournamentMatch, MAX_REPLAYS,
};
pub use service::EliminationService;
