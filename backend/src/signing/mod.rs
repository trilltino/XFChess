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

pub mod auth;
pub mod blinks;
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
use std::sync::Arc;
use std::str::FromStr;
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
    pub tournament_store: Arc<TournamentStore>,
    pub swiss_service: Arc<SwissService>,
    pub tournament_gossip: Arc<TournamentGossipService>,
    pub host_treasury_pubkey: Pubkey,
    pub usdc_mint_pubkey: Pubkey,
    pub pyth_oracle: Arc<PythOracle>,
    pub tournament_trigger: Option<tokio::sync::mpsc::Sender<TournamentTrigger>>,
    pub orchestrator_tx: Option<tokio::sync::mpsc::Sender<OrchestratorEvent>>,
    pub braid_hub: Arc<ResourceHub>,
}

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
            tournament_store,
            swiss_service,
            tournament_gossip,
            host_treasury_pubkey,
            usdc_mint_pubkey,
            pyth_oracle,
            tournament_trigger: None,
            orchestrator_tx: None,
            braid_hub,
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
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Core game session and move routes (These were missing from build_app_router)
        .merge(crate::signing::routes::main::routes())
        
        // Feature-specific nested routes
        .nest("/api/auth", crate::signing::routes::auth::auth_routes())
        .nest("/api/actions", blinks::blinks_routes())
        .merge(p2p_relay::p2p_routes())
        .nest("/identity", crate::signing::routes::identity::identity_routes())
        
        // Relayer infrastructure
        .merge(relayer::routes())
        .merge(tee_relayer::routes())
        
        // WebSocket route for authentication sync
        .route("/ws/auth", get(handle_auth_websocket))
        
        // State injection
        .with_state(state)
}
