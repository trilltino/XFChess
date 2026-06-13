//! Instructions managing multi-player structured tournaments.

pub mod lifecycle;
pub mod registration;
pub mod matches;
pub mod session;
pub mod prizes;

// Re-export all types from subdirectories for easier access
pub use lifecycle::{
    CancelTournament, CloseTournament, InitializeTournament, InitializeTournamentEscrow,
    InitializeTournamentShards, InitializeShardsSmall, InitializeShardsMedium, StartTournament,
};
pub use registration::{RegisterPlayer, LeaveTournament};
pub use matches::{
    AdvanceWinner, InitializeMatch, RecordMatchResult, RecordSwissResult, SwissMatchResult,
};
pub use session::{
    AuthorizeTournamentSessionArgs, AuthorizeTournamentSessionCtx, RevokeTournamentSessionCtx,
    SessionCreateGame, SessionJoinGame,
};
pub use prizes::{ClaimTournamentPrize, DistributeTournamentPrizes, FundSolPrize, FundUsdcPrize};
