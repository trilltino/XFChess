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
pub use client::{
    client, client_fast, fetch_sol_usd_rate, set_auth_token, vps_base, vps_ws_base,
    wallet_bridge_port, SolUsdRateResponse,
};
pub use game::{
    fetch_move_log, fetch_verified_participants, get_active_game_for_wallet, get_broadcast_delay,
    get_game_moves_for_spectator, record_move, report_blur, vps_delegate_game,
    vps_fetch_move_nonce, vps_finalize_game, vps_submit_dispute, vps_submit_free_rated_result,
    vps_undelegate_game,
};
pub use identity::{
    fetch_player_profile, get_user_status, get_user_status_async, link_wallet, register_identity,
    register_wallet, require_wager_eligibility, IdentityPayload, LinkWalletReq, PlayerProfile,
    RegisterReq, UserStatus,
};
pub use p2p::{
    p2p_accept_join, p2p_announce_game, p2p_announce_game_with_password, p2p_heartbeat,
    p2p_join_game, p2p_join_game_with_password, p2p_leave_game, p2p_leave_game_fast,
    p2p_list_games, p2p_list_games_filtered, p2p_poll_messages, p2p_send_message, P2PGameListing,
    P2PListFilter,
};
pub use session::{
    activate_session, create_session, fetch_platform_fee_lamports, session_status,
    track_global_session_game, verify_global_session, SessionStatus,
};
pub use social::{
    fetch_region, get_contacts, get_online, get_pending_requests, poll_social, push_lobby_invite,
    remove_contact, respond_friend_request, send_friend_request, update_presence,
    Contact as SocialContact, FriendRequest as SocialFriendRequest, LobbyInvite,
    Presence as SocialPresence, SocialPollResponse,
};
pub use tournament::{
    confirm_join, confirm_join_with_retry, fetch_game_pgn, fetch_tournament_games,
    list_tournament_games, list_tournaments, my_tournament_status, tournament_session_create_game,
    tournament_session_join_game, BlockedBy, GameState as TournamentGameState, LastMatchResult,
    MyTournamentStatus, PlayerState, TournamentGameListing, TournamentSummary,
};
