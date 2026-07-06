//! Account management instructions (profiles, user data, fees).

pub mod fee_vault_ix;
pub mod friends_ix;
pub mod global_session_ix;
pub mod link_external_elo;
pub mod profile;
pub mod profile_init;
pub mod session_guards;
pub mod set_username;
pub mod treasury;
pub mod withdraw;

pub use fee_vault_ix::{
    ClaimFees, CollectFee, CreateSession, InitializeFeeVault, RevokeSession, UpdateElo,
};
pub use friends_ix::{AcceptFriendRequest, BlockUser, CloseFriendship, SendFriendRequest};
pub use global_session_ix::{
    AuthorizeGlobalSessionArgs, AuthorizeGlobalSessionCtx, RevokeGlobalSessionCtx,
    WithdrawGlobalSessionCtx,
};
pub use link_external_elo::LinkExternalElo;
pub use profile::{InitProfile, VerifyProfile};
pub use set_username::SetUsername;
pub use treasury::WithdrawTreasury;
pub use withdraw::WithdrawExpiredWager;
