//! Tournament prize distribution instructions
//!
//! Instructions for prize claiming, funding, and SOL/USDC payouts.

pub mod claim_prize;
pub mod distribute;
pub mod fund_prize;
pub mod fund_sol_prize;

pub use claim_prize::ClaimTournamentPrize;
pub use distribute::DistributeTournamentPrizes;
pub use fund_prize::FundUsdcPrize;
pub use fund_sol_prize::FundSolPrize;
