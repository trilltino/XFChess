//! Tournament prize distribution instructions
//!
//! Instructions for prize claiming, funding, and SOL/USDC payouts.

pub mod claim_prize;
pub mod fund_prize;

pub use claim_prize::ClaimTournamentPrize;
pub use fund_prize::FundUsdcPrize;
