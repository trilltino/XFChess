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
#[path = "vps/game.rs"]
pub mod game;
#[path = "vps/identity.rs"]
pub mod identity;
#[path = "vps/p2p.rs"]
pub mod p2p;
#[path = "vps/session.rs"]
pub mod session;
#[path = "vps/social.rs"]
pub mod social;
#[path = "vps/tournament.rs"]
pub mod tournament;

// Re-exports preserving the flat `crate::multiplayer::network::vps::*` API.
pub use client::{client, fetch_sol_usd_rate, set_auth_token, vps_base, SolUsdRateResponse};
pub use game::{
    fetch_move_log, get_active_game_for_wallet, get_broadcast_delay, get_game_moves_for_spectator,
    record_move, report_blur, vps_fetch_move_nonce, vps_finalize_game, vps_submit_dispute,
    vps_submit_free_rated_result, vps_undelegate_game,
};
pub use identity::{
    fetch_player_profile, get_user_status, get_user_status_async, link_wallet, register_identity,
    register_wallet, require_wager_eligibility, IdentityPayload, LinkWalletReq, PlayerProfile,
    RegisterReq, UserStatus,
};
pub use p2p::{
    p2p_announce_game, p2p_announce_game_with_password, p2p_heartbeat, p2p_join_game,
    p2p_join_game_with_password, p2p_leave_game, p2p_list_games, p2p_list_games_filtered,
    p2p_poll_messages, p2p_send_message, P2PGameListing, P2PListFilter,
};
#[cfg(feature = "solana")]
pub use session::tee_authenticate;
pub use session::{
    activate_session, create_session, session_status, sign_and_submit, verify_global_session,
    SessionStatus,
};
pub use social::{
    fetch_region, get_contacts, get_online, get_pending_requests, poll_social, push_lobby_invite,
    remove_contact, respond_friend_request, send_friend_request, update_presence,
    Contact as SocialContact, FriendRequest as SocialFriendRequest, LobbyInvite,
    Presence as SocialPresence, SocialPollResponse,
};
pub use tournament::{
    join_tournament, list_tournament_games, list_tournaments, tournament_session_create_game,
    tournament_session_join_game, TournamentGameListing, TournamentSummary,
};
