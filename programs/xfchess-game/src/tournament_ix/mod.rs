//! Instructions managing multi-player structured tournaments.

pub mod lifecycle;
pub mod matches;
pub mod prizes;
pub mod registration;
pub mod session;
pub mod shards;

// Re-export all types from subdirectories for easier access
pub use lifecycle::{
    CancelTournament, CloseTournament, InitializeShardsMedium, InitializeShardsSmall,
    InitializeTournament, InitializeTournamentEscrow, InitializeTournamentShards, StartTournament,
};
pub use matches::{
    AdvanceRound, AdvanceWinner, InitializeMatch, RecordMatchResult, RecordSwissResult,
    SwissMatchResult,
};
pub use prizes::{ClaimTournamentPrize, DistributeTournamentPrizes, FundSolPrize, FundUsdcPrize};
pub use registration::{LeaveTournament, RegisterPlayer};
pub use session::{
    AuthorizeTournamentSessionArgs, AuthorizeTournamentSessionCtx, RevokeTournamentSessionCtx,
    SessionCreateGame, SessionJoinGame,
};
