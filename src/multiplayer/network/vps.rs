#![allow(dead_code)]
//! Blocking HTTP client for the XFChess signing-server VPS.
//!
//! This module is a thin facade that re-exports the feature-grouped
//! submodules under `vps/`. All public helpers below are preserved for
//! backward compatibility with existing call sites such as
//! `crate::multiplayer::vps_client::*`.
//!
//! Every public function is synchronous `reqwest` and is intended to run
//! inside a Bevy `IoTaskPool` task or `tokio::task::spawn_blocking`.
//!
//! Submodules:
//! - [`client`] — shared HTTP client + base URL resolution
//! - [`session`] — session create/activate/status/sign + TEE auth
//! - [`game`] — move recording, undelegate, finalize
//! - [`identity`] — player profile, KYC, status, eligibility gates
//! - [`tournament`] — tournament listing and joining
//! - [`p2p`] — P2P relay (announce / list / join / message / poll / leave)

#[path = "vps/client.rs"]
mod client;
#[path = "vps/session.rs"]
pub mod session;
#[path = "vps/game.rs"]
pub mod game;
#[path = "vps/identity.rs"]
pub mod identity;
#[path = "vps/tournament.rs"]
pub mod tournament;
#[path = "vps/p2p.rs"]
pub mod p2p;

// Re-exports preserving the flat `crate::multiplayer::network::vps::*` API.
pub use client::{fetch_sol_gbp_rate, vps_base, SolGbpRateResponse};
pub use session::{
    activate_session, create_session, session_status, sign_and_submit, SessionStatus,
};
#[cfg(feature = "solana")]
pub use session::tee_authenticate;
pub use game::{record_move, vps_finalize_game, vps_undelegate_game};
pub use identity::{
    fetch_player_profile, get_user_status, get_user_status_async, link_wallet, register_identity,
    register_wallet, require_wager_eligibility, IdentityPayload, LinkWalletReq, PlayerProfile,
    RegisterReq, UserStatus,
};
pub use tournament::{join_tournament, list_tournaments, TournamentSummary};
pub use p2p::{
    p2p_announce_game, p2p_join_game, p2p_leave_game, p2p_list_games, p2p_poll_messages,
    p2p_send_message, P2PGameListing,
};
