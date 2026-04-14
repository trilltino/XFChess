//! XFChess signing service module.
//!
//! This module provides the backend signing service for the XFChess game.
//! It handles:
//! - JWT-based authentication for wallet owners
//! - Session key management for game transactions
//! - Solana instruction building and transaction signing
//! - P2P game relay for NAT traversal
//! - Tournament bracket management
//! - Identity vault for encrypted KYC data
//! - Matchmaking queue for player matching
//!
//! # Module Organization
//!
//! - `auth`: JWT token issuance and verification
//! - `config`: Environment configuration
//! - `feepayer`: Fee-payer keypair pool for transactions
//! - `identity`: Identity vault for encrypted KYC data
//! - `p2p_relay`: P2P game relay for NAT traversal
//! - `routes`: HTTP route handlers (organized by feature)
//! - `solana`: Solana instruction builders and RPC helpers
//! - `storage`: SQLite-backed data stores

pub mod auth;
pub mod cacf_compliance;
pub mod config;
pub mod elo_cache;
pub mod feepayer;
pub mod identity;
pub mod p2p_relay;
pub mod routes;
pub mod solana;
pub mod storage;

use axum::{
    routing::{get, post},
    Router,
};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::str::FromStr;

pub use auth::JwtIssuer;
pub use config::SigningConfig;
pub use elo_cache::EloCache;
pub use feepayer::FeepayerPool;
pub use identity::IdentityVault;
pub use routes::matchmaking::SharedMatchmakingState;
pub use storage::{SessionStore, tournament::TournamentStore};

/// Shared state injected into every route handler.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<SigningConfig>,
    pub store: Arc<SessionStore>,
    pub feepayer: Arc<FeepayerPool>,
    pub jwt: Arc<JwtIssuer>,
    pub matchmaking: SharedMatchmakingState,
    pub identity_vault: Arc<IdentityVault>,
    pub p2p_relay: Arc<p2p_relay::P2PRelayState>,
    pub vault_pool: Arc<sqlx::SqlitePool>,
    pub elo_cache: Arc<EloCache>,
    pub matchmaking_state: SharedMatchmakingState,
}

impl AppState {
    pub fn new(config: SigningConfig, pool: sqlx::SqlitePool, vault_pool: sqlx::SqlitePool) -> Self {
        let store = Arc::new(storage::SessionStore::new(pool.clone()));
        let feepayer = Arc::new(feepayer::FeepayerPool::from_base58_list(&config.fee_payer_keys));
        let jwt = Arc::new(auth::JwtIssuer::new(&config.jwt_secret));
        let matchmaking = routes::matchmaking::SharedMatchmakingState::default();
        let matchmaking_state = routes::matchmaking::SharedMatchmakingState::default();

        let identity_vault = identity::IdentityVault::new(
            &config.identity_encryption_key,
            &config.identity_salt
        ).expect("Failed to initialize IdentityVault from env config");
        
        let p2p_relay = Arc::new(p2p_relay::create_relay_state());
        
        // Initialize ELO cache with 5-minute TTL
        let program_id = Pubkey::from_str(&config.program_id)
            .expect("Invalid program_id in config");
        let elo_cache = Arc::new(EloCache::new(
            config.solana_rpc_url.clone(),
            std::time::Duration::from_secs(300),
            program_id,
        ));
        
        Self {
            config: Arc::new(config),
            store,
            feepayer,
            jwt,
            matchmaking,
            identity_vault: Arc::new(identity_vault),
            p2p_relay,
            vault_pool: Arc::new(vault_pool),
            elo_cache,
            matchmaking_state,
        }
    }
}

/// Builds the Axum router with all signing service routes.
///
/// # Arguments
/// * `state` - The shared application state
///
/// # Returns
/// An Axum Router with all route handlers registered
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Auth routes
        .nest("/api/auth", routes::auth::auth_routes())
        // Main API routes (sessions, moves, games)
        .route("/auth/issue", post(routes::main::issue_jwt))
        .route("/session/create", post(routes::main::create_session))
        .route("/session/activate", post(routes::main::activate_session))
        .route("/session/status/{game_id}", get(routes::main::session_status))
        .route("/session/sign", post(routes::main::sign_tx))
        .route("/move/record", post(routes::main::record_move))
        .route("/game/undelegate", post(routes::main::undelegate_game))
        .route("/game/finalize", post(routes::main::finalize_game))
        .route("/stats", get(routes::main::get_stats))
        // P2P relay routes
        .route("/p2p/announce", post(p2p_relay::announce_game))
        .route("/p2p/games", get(p2p_relay::list_games))
        .route("/p2p/join", post(p2p_relay::join_game))
        .route("/p2p/accept", post(p2p_relay::accept_join))
        .route("/p2p/leave", post(p2p_relay::leave_game))
        .route("/p2p/message", post(p2p_relay::send_message))
        .route("/p2p/poll", post(p2p_relay::poll_messages))
        // Identity routes
        .nest("/identity", routes::identity::identity_routes())
        .with_state(state)
}
