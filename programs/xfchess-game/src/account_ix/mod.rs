//! Account management instructions (profiles, user data, fees).

pub mod profile;
pub mod set_username;
pub mod withdraw;
pub mod fee_vault_ix;

pub use profile::{InitProfile, VerifyProfile};
pub use set_username::SetUsername;
pub use withdraw::WithdrawExpiredWager;
pub use fee_vault_ix::{
    InitializeFeeVault, CollectFee, ClaimFees,
    CreateSession, RevokeSession, UpdateElo,
};
