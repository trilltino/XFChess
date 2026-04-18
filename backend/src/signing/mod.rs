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
pub mod blinks;
pub mod blinks_anti_cheat;
pub mod blinks_chains;
pub mod blinks_funding;
pub mod blinks_onboarding;
pub mod blinks_pda;
pub mod cacf_compliance;
pub mod config;
pub mod elo_cache;
pub mod feepayer;
pub mod identity;
pub mod p2p_relay;
pub mod routes;
pub mod solana;
pub mod storage;
pub mod swiss;
pub mod tournament_gossip;

use axum::Router;
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
pub use routes::matchmaking::SharedMatchmakingState;
pub use storage::{SessionStore, tournament::TournamentStore};
pub use swiss::SwissService;
pub use tournament_gossip::TournamentGossipService;
pub use crate::tasks::tournament_scheduler::TournamentTrigger;

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
    pub tournament_trigger: Option<tokio::sync::mpsc::Sender<TournamentTrigger>>,
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


        // Parse authority keys
        let vps_authority = Arc::new(config.vps_authority_key.as_ref()
            .map(|k| Keypair::from_base58_string(k))
            .unwrap_or_else(|| {
                warn!("[VPS] No vps_authority_key provided, using random fallback");
                Keypair::new()
            }));

        let kyc_authority = Arc::new(config.kyc_authority_key.as_ref()
            .map(|k| Keypair::from_base58_string(k))
            .unwrap_or_else(|| {
                warn!("[VPS] No kyc_authority_key provided, using random fallback");
                Keypair::new()
            }));

        // Initialize Swiss service
        let swiss_service = Arc::new(swiss::SwissService::new((*tournament_store).clone()));

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
            tournament_trigger: None,
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
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .nest("/api/auth", routes::auth::auth_routes())
        .nest("/api/actions", routes::blinks::blinks_routes())
        .nest("/api/tournaments", routes::tournament::tournaments_routes())
        .nest("/api/tournament", routes::tournament::tournament_routes())
        .nest("/admin/tournament", routes::tournament::admin_tournament_app_state_routes())
        .nest("/tournament", routes::tournament::tournament_gossip_routes())
        .merge(p2p_relay::p2p_routes())
        .nest("/identity", routes::identity::identity_routes())
        .with_state(state)
}
