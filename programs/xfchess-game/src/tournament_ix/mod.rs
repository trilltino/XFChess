//! Instructions managing multi-player structured tournaments.

pub mod initialize;
pub mod initialize_match;
pub mod register;
pub mod start;
pub mod record_result;
pub mod record_swiss_result;
pub mod claim_prize;
pub mod claim_streaming;
pub mod cancel;
pub mod fund_prize;

pub use initialize::InitializeTournament;
pub use initialize_match::InitializeMatch;
pub use register::RegisterPlayer;
pub use start::StartTournament;
pub use record_result::{AdvanceWinner, RecordMatchResult};
pub use record_swiss_result::{RecordSwissResult, SwissMatchResult};
pub use claim_prize::ClaimTournamentPrize;
pub use claim_streaming::ClaimStreamingPrize;
pub use cancel::CancelTournament;
pub use fund_prize::FundUsdcPrize;
