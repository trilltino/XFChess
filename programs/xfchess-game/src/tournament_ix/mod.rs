//! Instructions managing multi-player structured tournaments.

pub mod initialize;
pub mod initialize_match;
pub mod register;
pub mod start;
pub mod record_result;
pub mod claim_prize;
pub mod cancel;

pub use initialize::InitializeTournament;
pub use initialize_match::InitializeMatch;
pub use register::RegisterPlayer;
pub use start::StartTournament;
pub use record_result::{AdvanceWinner, RecordMatchResult};
pub use claim_prize::ClaimTournamentPrize;
pub use cancel::CancelTournament;
