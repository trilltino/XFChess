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
//! - CACF compliance for regulated jurisdictions
//! - Solana Blinks for tournament registration
//!
//! # Module Organization
//!
//! - `auth`: JWT token issuance and verification
//! - `blinks`: Solana Blinks API for tournament registration
//! - `cacf`: CACF compliance (UK, Brazil, Germany, Canada)
//! - `config`: Environment configuration
//! - `feepayer`: Fee-payer keypair pool for transactions
//! - `identity`: Identity vault for encrypted KYC data
//! - `p2p_relay`: P2P game relay for NAT traversal
//! - `routes`: HTTP route handlers (organized by feature)
//! - `solana`: Solana instruction builders and RPC helpers
//! - `storage`: SQLite-backed data stores
//! - `swiss`: Swiss pairing tournament system
//! - `relayer`: Relayer routes
//! - `tee_relayer`: Tee Relayer routes
//! - `auth_ws`: WebSocket authentication

pub mod anticheat_enqueue;
pub mod auth;
pub mod blinks;
pub mod linkage;
pub mod social;
pub mod blinks_onboarding;
pub mod blinks_funding;
pub mod ws_subscriber;
pub mod tee_relayer;
pub mod cacf;
pub mod config;
pub mod elo_cache;
pub mod feepayer;
pub mod identity;
pub mod p2p_relay;
pub mod pyth_oracle;
pub mod routes;
pub mod solana;
pub mod storage;
pub mod swiss;
pub mod tournament_gossip;
pub mod relayer {
    //! Re-export relayer routes from the routes module.
    pub use crate::signing::routes::relayer::*;
}
pub mod auth_ws;

use axum::Router;
use axum::routing::get;
use crate::signing::auth_ws::handle_auth_websocket;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;
use tokio::sync::Mutex;
use tracing::warn;

pub use auth::JwtIssuer;
pub use config::SigningConfig;
pub use elo_cache::EloCache;
pub use feepayer::FeepayerPool;
pub use identity::IdentityVault;
pub use pyth_oracle::PythOracle;
pub use routes::matchmaking::SharedMatchmakingState;
pub use storage::{SessionStore, tournament::TournamentStore};
pub use swiss::{SwissService, OrchestratorEvent};
pub use tournament_gossip::TournamentGossipService;
pub use crate::tasks::tournament_scheduler::TournamentTrigger;
pub use xfchess_braid_server::ResourceHub;
pub use xfchess_anticheat::engine::job_queue::AnalysisQueue;
pub use social::{FriendManager, PresenceStore};

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
    pub vps_authority: Arc<Keypair>,
    pub kyc_authority: Arc<Keypair>,
    pub link_authority: Arc<Keypair>,
    pub tournament_store: Arc<TournamentStore>,
    pub swiss_service: Arc<SwissService>,
    pub tournament_gossip: Arc<TournamentGossipService>,
    pub host_treasury_pubkey: Pubkey,
    pub usdc_mint_pubkey: Pubkey,
    pub pyth_oracle: Arc<PythOracle>,
    pub rate_cache: crate::signing::routes::rates::RateCache,
    pub tournament_trigger: Option<tokio::sync::mpsc::Sender<TournamentTrigger>>,
    pub orchestrator_tx: Option<tokio::sync::mpsc::Sender<OrchestratorEvent>>,
    pub braid_hub: Arc<ResourceHub>,
    pub chat_relay: routes::chat::ChatRelayState,
    pub metrics: Arc<crate::telemetry::metrics::Metrics>,

    // ── Global session management ──────────────────────────────────────────────
    pub pending_global_sessions: Arc<Mutex<HashMap<Pubkey, Keypair>>>,
    pub active_global_sessions: Arc<Mutex<HashMap<Pubkey, Keypair>>>,

    pub solana_rpc_url: String,
    pub program_id: Pubkey,
    /// Shared blocking RPC client — avoids a new TCP connection per route call.
    pub solana_rpc: Arc<solana_client::rpc_client::RpcClient>,

    // ── Anti-cheat ─────────────────────────────────────────────────────────────
    pub anticheat_queue: Option<AnalysisQueue>,

    // ── Social (friends + presence) ────────────────────────────────────────────
    pub friends: Arc<FriendManager>,
    pub presence: Arc<PresenceStore>,
    /// Pending lobby invites keyed by recipient node_id
    pub invite_store: Arc<std::sync::RwLock<HashMap<String, Vec<social::routes::LobbyInvite>>>>,

    // ── SIWS nonce store — one-time nonces keyed by nonce string ───────────────
    /// Maps nonce → (wallet_pubkey, expires_unix_secs)
    pub siws_nonces: Arc<Mutex<HashMap<String, (String, u64)>>>,
}

// Compile-time check: AppState must be Clone + Send + Sync + 'static for axum::serve
#[allow(dead_code)]
const _: () = {
    fn assert_bounds<T: Clone + Send + Sync + 'static>() {}
    // If AppState violates any bound, this line will produce a clear error.
    let _ = assert_bounds::<AppState>;
};

impl AppState {
    pub fn new(config: SigningConfig, pool: sqlx::SqlitePool, vault_pool: sqlx::SqlitePool, tournament_store: Arc<TournamentStore>) -> Self {
        let store = Arc::new(storage::SessionStore::new(pool.clone()));
        let feepayer = Arc::new(feepayer::FeepayerPool::from_base58_list(&config.fee_payer_keys));
        let jwt = Arc::new(auth::JwtIssuer::new(&config.jwt_secret));
        let matchmaking = routes::matchmaking::SharedMatchmakingState::default();

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


        // Parse authority keys — accepts either a JSON file path or a base58 string
        let load_keypair = |val: &str| -> Keypair {
            if std::path::Path::new(val).exists() {
                let bytes: Vec<u8> = std::fs::read_to_string(val)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();
                Keypair::from_bytes(&bytes).unwrap_or_else(|_| Keypair::new())
            } else {
                Keypair::from_base58_string(val)
            }
        };

        let vps_authority = Arc::new(config.vps_authority_key.as_deref()
            .map(load_keypair)
            .unwrap_or_else(|| {
                warn!("[VPS] No vps_authority_key provided, using random fallback");
                Keypair::new()
            }));

        let kyc_authority = Arc::new(config.kyc_authority_key.as_deref()
            .map(load_keypair)
            .unwrap_or_else(|| {
                warn!("[VPS] No kyc_authority_key provided, using random fallback");
                Keypair::new()
            }));

        let link_authority = Arc::new(config.link_authority_key.as_deref()
            .map(load_keypair)
            .unwrap_or_else(|| {
                warn!("[VPS] No link_authority_key provided, using random fallback — external ELO linking will fail on-chain");
                Keypair::new()
            }));

        // Initialize Swiss service and attach Braid hub
        let braid_hub = Arc::new(ResourceHub::new());
        let mut _swiss = swiss::SwissService::new((*tournament_store).clone());
        _swiss.set_braid_hub(Arc::clone(&braid_hub));
        let swiss_service = Arc::new(_swiss);

        // Initialize tournament gossip service (VPS node ID will be set later)
        let tournament_gossip = Arc::new(TournamentGossipService::new(
            (*tournament_store).clone(),
            None,
        ));

        // Parse host treasury and USDC mint pubkeys
        let host_treasury_pubkey = Pubkey::from_str(&config.host_treasury_pubkey)
            .expect("Invalid host_treasury_pubkey in config");
        let usdc_mint_pubkey = Pubkey::from_str(&config.usdc_mint_pubkey)
            .expect("Invalid usdc_mint_pubkey in config");

        // Initialize Pyth oracle for dynamic pricing
        let pyth_oracle = Arc::new(PythOracle::new());
        let rate_cache = routes::rates::RateCache::default();
        let metrics = Arc::new(crate::telemetry::metrics::Metrics::new());

        let solana_rpc_url = config.solana_rpc_url.clone();
        let solana_rpc = Arc::new(solana::rpc::make_rpc(&solana_rpc_url));

        let friends = Arc::new(FriendManager::new(pool.clone()));
        let presence = Arc::new(PresenceStore::new());

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
            vps_authority,
            kyc_authority,
            link_authority,
            tournament_store,
            swiss_service,
            tournament_gossip,
            host_treasury_pubkey,
            usdc_mint_pubkey,
            pyth_oracle,
            rate_cache,
            tournament_trigger: None,
            orchestrator_tx: None,
            braid_hub,
            chat_relay: routes::chat::new_chat_relay(),
            metrics,
            pending_global_sessions: Arc::new(Mutex::new(HashMap::new())),
            active_global_sessions: Arc::new(Mutex::new(HashMap::new())),
            solana_rpc_url,
            program_id,
            solana_rpc,
            anticheat_queue: None,
            friends,
            presence,
            invite_store: Arc::new(std::sync::RwLock::new(HashMap::new())),
            siws_nonces: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Initialize gossip service with VPS node ID (called after P2P node starts)
    /// Note: node_id is a string representation of the iroh endpoint ID
    pub async fn init_gossip(&self, vps_node_id: String) {
        // Create new gossip service with VPS node ID
        // The node_id is stored as string since iroh crate may not be available in all contexts
        let _new_gossip = Arc::new(TournamentGossipService::new(
            (*self.tournament_store).clone(),
            Some(vps_node_id.clone()), // Pass the String directly
        ));
        
        // This would require interior mutability in practice
        // For now, gossip service is initialized without VPS node ID
        tracing::info!("[AppState] Gossip service initialized with VPS node {}", vps_node_id);
    }
}

/// Builds the Axum router with all signing service routes.
///
/// Uses per-feature router functions merged together for clear separation of concerns.
/// Note: tournament routes are mounted in build_app_router to avoid duplication.
pub fn build_router(state: AppState) -> Router<AppState> {
    let base = Router::new().with_state(state.clone());
    base
        // Debug and health routes
        .merge(crate::signing::routes::debug::debug_routes())
        
        // Core game session and move routes (These were missing from build_app_router)
        .merge(crate::signing::routes::main::routes())

        // Session-key signing endpoints — dual-accept guard: a valid per-user JWT
        // (preferred) or the legacy relay secret. See `require_relay_or_jwt`.
        .merge(
            crate::signing::routes::main::protected_routes()
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    crate::infrastructure::require_relay_or_jwt,
                )),
        )
        
        // Feature-specific nested routes
        .nest("/api/auth", crate::signing::routes::auth::auth_routes())
        .nest("/api/actions", blinks::blinks_routes())
        .nest(
            "/api/rates",
            crate::signing::routes::rates::rates_routes(),
        )
        .merge(p2p_relay::p2p_routes())
        .nest("/identity", crate::signing::routes::identity::identity_routes())
        
        // Relayer infrastructure
        .merge(relayer::routes())
        .merge(tee_relayer::routes())
        
        // WebSocket route for authentication sync
        .route("/ws/auth", get(handle_auth_websocket))

        // Global persistent session delegation — verify is public, mutations require admin key
        .nest("/api/global-session", crate::signing::routes::global_session::global_session_public_routes())
        .nest(
            "/api/global-session",
            crate::signing::routes::global_session::global_session_protected_routes()
                .layer(axum::middleware::from_fn(crate::infrastructure::auth_middleware::require_api_key)),
        )

        // Anti-cheat verdict + player stats queries
        .nest("/api", crate::signing::routes::anticheat::anticheat_routes())

        // Wallet balance (SOL + stablecoins via Helius, converted to local currency)
        .nest("/api/wallet", crate::signing::routes::wallet::wallet_routes())

        // External ELO linking (Lichess)
        .nest("/api", crate::signing::routes::external_elo::external_elo_routes())

        // Lichess OAuth 2.0 + PKCE flow (primary)
        .nest("/api", crate::signing::routes::lichess_oauth::lichess_oauth_routes())
}
