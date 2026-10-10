pub mod anticheat_enqueue;
pub mod auth;
pub mod auth_ws;
pub mod blinks;
pub mod config;
pub mod elo_cache;
pub mod feepayer;
pub mod game_pgn;
pub mod identity;
pub mod linkage;
pub mod p2p_relay;
pub mod privy;
pub mod routes;
pub mod social;
pub mod solana;
pub mod storage;
pub mod swiss;
pub mod tournament_gossip;
pub mod tournament_operations;
pub mod ws_subscriber;

use crate::signing::auth_ws::handle_auth_websocket;
use axum::routing::get;
use axum::Router;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

pub use crate::tasks::tournament_scheduler::TournamentTrigger;
pub use auth::JwtIssuer;
pub use config::SigningConfig;
pub use elo_cache::EloCache;
pub use feepayer::FeepayerPool;
pub use identity::IdentityVault;
pub use routes::matchmaking::SharedMatchmakingState;
pub use social::{FriendManager, PresenceStore};
pub use storage::{tournament::TournamentStore, SessionStore};
pub use swiss::{OrchestratorEvent, SwissService};
pub use tournament_gossip::TournamentGossipService;
pub use xfchess_anticheat::engine::job_queue::AnalysisQueue;
pub use xfchess_braid_server::ResourceHub;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<SigningConfig>,
    pub store: Arc<SessionStore>,
    pub money_actions: Arc<storage::money_action::MoneyActionStore>,
    pub feepayer: Arc<FeepayerPool>,
    pub jwt: Arc<JwtIssuer>,
    pub matchmaking: SharedMatchmakingState,
    pub identity_vault: Arc<IdentityVault>,
    pub p2p_relay: Arc<p2p_relay::P2PRelayState>,
    pub p2p_relay_store: p2p_relay::RelayStore,
    pub seat_leases: storage::seat_lease::SeatLeaseStore,
    pub vault_pool: Arc<sqlx::SqlitePool>,
    pub elo_cache: Arc<EloCache>,
    pub vps_authority: Arc<Keypair>,
    pub kyc_authority: Arc<Keypair>,
    pub link_authority: Arc<Keypair>,
    pub treasury_authority_pubkey: Pubkey,
    pub tournament_store: Arc<TournamentStore>,
    pub swiss_service: Arc<SwissService>,
    pub tournament_gossip: Arc<TournamentGossipService>,
    pub tournament_fee_recipient: Pubkey,
    pub usdc_mint_pubkey: Pubkey,
    pub rate_cache: crate::signing::routes::rates::RateCache,
    pub tournament_trigger: Option<tokio::sync::mpsc::Sender<TournamentTrigger>>,
    pub orchestrator_tx: Option<tokio::sync::mpsc::Sender<OrchestratorEvent>>,
    pub braid_hub: Arc<ResourceHub>,
    pub game_log: Arc<routes::game_log::GameLogState>,
    pub metrics: Arc<crate::telemetry::metrics::Metrics>,

    pub active_global_sessions: Arc<Mutex<HashMap<Pubkey, Keypair>>>,

    pub er_write_locks: Arc<Mutex<HashMap<u64, Arc<tokio::sync::Mutex<()>>>>>,

    pub solana_rpc_url: String,
    pub program_id: Pubkey,
    pub solana_rpc: Arc<solana_client::rpc_client::RpcClient>,
    pub game_participants: solana::game_participants::GameParticipantsCache,

    pub anticheat_queue: Option<AnalysisQueue>,

    pub friends: Arc<FriendManager>,
    pub presence: Arc<PresenceStore>,
    pub invite_store: Arc<std::sync::RwLock<HashMap<String, Vec<social::routes::LobbyInvite>>>>,

    pub siws_nonces: Arc<Mutex<HashMap<String, (String, u64)>>>,
}

// Compile-time check: AppState must be Clone + Send + Sync + 'static for axum::serve
#[allow(dead_code)]
const _: () = {
    fn assert_bounds<T: Clone + Send + Sync + 'static>() {}
    // If AppState violates any bound, this line will produce a clear error.
    let _ = assert_bounds::<AppState>;
};

pub fn load_keypair_from_env_value(val: &str) -> Result<Keypair, String> {
    if std::path::Path::new(val).exists() {
        let contents = std::fs::read_to_string(val)
            .map_err(|e| format!("could not read keyfile '{val}': {e}"))?;
        let bytes: Vec<u8> = serde_json::from_str(&contents)
            .map_err(|e| format!("keyfile '{val}' is not a valid JSON keypair array: {e}"))?;
        Keypair::try_from(bytes.as_slice())
            .map_err(|e| format!("keyfile '{val}' does not contain a valid ed25519 keypair: {e}"))
    } else {
        // Malformed base58 panics; keyfile errors return Result.
        Ok(Keypair::from_base58_string(val))
    }
}

impl AppState {
    pub fn new(
        config: SigningConfig,
        pool: sqlx::SqlitePool,
        vault_pool: sqlx::SqlitePool,
        tournament_store: Arc<TournamentStore>,
    ) -> Self {
        // Initialize the encryption vault before the store so persisted session keys
        // are encrypted at rest.
        let identity_vault =
            identity::IdentityVault::new(&config.identity_encryption_key, &config.identity_salt)
                .expect("Failed to initialize IdentityVault from env config");

        let store = Arc::new(storage::SessionStore::new(
            pool.clone(),
            identity_vault.clone(),
        ));
        let money_actions = Arc::new(storage::money_action::MoneyActionStore::new(pool.clone()));
        let feepayer = Arc::new(feepayer::FeepayerPool::from_base58_list(
            &config.fee_payer_keys,
        ));
        let jwt = Arc::new(auth::JwtIssuer::new(&config.jwt_secret));

        let p2p_relay_store = p2p_relay::RelayStore::new(pool.clone());
        let p2p_relay = Arc::new(p2p_relay::create_relay_state(Some(p2p_relay_store.clone())));

        let program_id =
            Pubkey::from_str(&config.program_id).expect("Invalid program_id in config");
        let elo_cache = Arc::new(EloCache::new(
            config.solana_rpc_url.clone(),
            std::time::Duration::from_secs(300),
            program_id,
        ));
        // Share the ELO cache and persist matchmaking state in the session pool.
        let matchmaking =
            routes::matchmaking::SharedMatchmakingState::new(elo_cache.clone(), pool.clone());

        // Accept JSON keyfiles or base58 keys. Malformed authority keys are fatal
        // in production; development may generate temporary keys.
        let is_production = config.is_production();
        let resolve_authority = |name: &str, key: &Option<String>| -> Keypair {
            match key.as_deref().map(load_keypair_from_env_value) {
                Some(Ok(kp)) => kp,
                Some(Err(e)) if is_production => {
                    panic!("[VPS] {name} is set but invalid in production, refusing to start: {e}")
                }
                Some(Err(e)) => {
                    warn!("[VPS] {name} is set but invalid ({e}), using random fallback");
                    Keypair::new()
                }
                None if is_production => {
                    panic!("[VPS] {name} not provided in production — this should have been caught by SigningConfig::validate()")
                }
                None => {
                    warn!("[VPS] No {name} provided, using random fallback");
                    Keypair::new()
                }
            }
        };

        let vps_authority = Arc::new(resolve_authority(
            "vps_authority_key",
            &config.vps_authority_key,
        ));
        let kyc_authority = Arc::new(resolve_authority(
            "kyc_authority_key",
            &config.kyc_authority_key,
        ));
        let link_authority = Arc::new(resolve_authority(
            "link_authority_key",
            &config.link_authority_key,
        ));
        // Not loaded via `resolve_authority` — no secret key involved. See
        // `treasury_authority_pubkey`'s field doc.
        let treasury_authority_pubkey = Pubkey::from_str(&config.treasury_authority_pubkey)
            .unwrap_or_else(|e| {
                panic!(
                    "Invalid treasury_authority_pubkey in config ({}): {e} — \
                     this should have been caught by SigningConfig::validate()",
                    config.treasury_authority_pubkey
                )
            });

        // Resolved authority pubkeys must match the program constants or privileged
        // instructions fail with UnauthorizedAccess.
        tracing::info!(
            "[VPS] Authority pubkeys — vps: {}, kyc: {}, link: {}, treasury: {} (pubkey only, \
             signing key never loaded here — see bin/treasury_signer.rs)",
            vps_authority.pubkey(),
            kyc_authority.pubkey(),
            link_authority.pubkey(),
            treasury_authority_pubkey,
        );

        let braid_hub = Arc::new(ResourceHub::new());
        let mut _swiss = swiss::SwissService::new((*tournament_store).clone());
        _swiss.set_braid_hub(Arc::clone(&braid_hub));
        let swiss_service = Arc::new(_swiss);

        let tournament_gossip = Arc::new(TournamentGossipService::new(
            (*tournament_store).clone(),
            None,
        ));

        let tournament_fee_recipient = Pubkey::from_str(&config.tournament_fee_recipient)
            .expect("Invalid tournament_fee_recipient in config");
        let usdc_mint_pubkey =
            Pubkey::from_str(&config.usdc_mint_pubkey).expect("Invalid usdc_mint_pubkey in config");

        // Refresh fee exchange rates in the background so payment requests do not wait on external feeds.
        let rate_cache = routes::rates::RateCache::default();
        rate_cache.spawn_background_refresh();
        let metrics = Arc::new(crate::telemetry::metrics::Metrics::new());

        let solana_rpc_url = config.solana_rpc_url.clone();
        let solana_rpc = Arc::new(solana::rpc::make_rpc(&solana_rpc_url));
        let game_participants =
            solana::game_participants::GameParticipantsCache::new(solana_rpc.clone(), program_id);
        let game_log = Arc::new(routes::game_log::GameLogState::new(
            pool.clone(),
            Some(game_participants.clone()),
        ));

        let friends = Arc::new(FriendManager::new(pool.clone()));
        let presence = Arc::new(PresenceStore::new());

        let invite_store = Arc::new(std::sync::RwLock::new(HashMap::new()));
        social::routes::spawn_invite_store_sweep(invite_store.clone());

        Self {
            config: Arc::new(config),
            store,
            money_actions,
            feepayer,
            jwt,
            matchmaking,
            identity_vault: Arc::new(identity_vault),
            p2p_relay,
            p2p_relay_store,
            seat_leases: storage::seat_lease::SeatLeaseStore::new(pool.clone()),
            vault_pool: Arc::new(vault_pool),
            elo_cache,
            vps_authority,
            kyc_authority,
            link_authority,
            treasury_authority_pubkey,
            tournament_store,
            swiss_service,
            tournament_gossip,
            tournament_fee_recipient,
            usdc_mint_pubkey,
            rate_cache,
            tournament_trigger: None,
            orchestrator_tx: None,
            braid_hub,
            game_log,
            metrics,
            active_global_sessions: Arc::new(Mutex::new(HashMap::new())),
            er_write_locks: Arc::new(Mutex::new(HashMap::new())),
            solana_rpc_url,
            program_id,
            solana_rpc,
            game_participants,
            anticheat_queue: None,
            friends,
            presence,
            invite_store,
            siws_nonces: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn spawn_state_sweeps(state: AppState) {
        const SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SWEEP_INTERVAL);
            interval.tick().await; // skip the immediate first tick
            loop {
                interval.tick().await;

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                {
                    let mut nonces = state.siws_nonces.lock().await;
                    let before = nonces.len();
                    nonces.retain(|_, (_, expires_at)| *expires_at > now);
                    let dropped = before.saturating_sub(nonces.len());
                    if dropped > 0 {
                        tracing::debug!(
                            "[sweep] dropped {dropped} expired SIWS nonces ({} live)",
                            nonces.len()
                        );
                    }
                }

                // Remove a game lock only when the map holds its sole Arc reference;
                // no caller then owns or waits on that critical section.
                {
                    let mut locks = state.er_write_locks.lock().await;
                    let before = locks.len();
                    locks.retain(|_, lock| Arc::strong_count(lock) > 1);
                    let dropped = before.saturating_sub(locks.len());
                    if dropped > 0 {
                        tracing::debug!(
                            "[sweep] released {dropped} idle ER game locks ({} live)",
                            locks.len()
                        );
                    }
                }

                match state.money_actions.retryable(25).await {
                    Ok(actions) => {
                        for action in actions {
                            let state_clone = state.clone();
                            tokio::spawn(async move {
                                routes::money_actions::reconcile_money_action_once(
                                    state_clone,
                                    action.id,
                                )
                                .await;
                            });
                        }
                    }
                    Err(e) => tracing::warn!("[sweep] money action retry scan failed: {}", e),
                }
            }
        });
    }

    pub async fn init_gossip(&self, vps_node_id: String) {
        // Create new gossip service with VPS node ID
        // The node_id is stored as string since iroh crate may not be available in all contexts
        let _new_gossip = Arc::new(TournamentGossipService::new(
            (*self.tournament_store).clone(),
            Some(vps_node_id.clone()), // Pass the String directly
        ));

        // This would require interior mutability in practice
        // For now, gossip service is initialized without VPS node ID
        tracing::info!(
            "[AppState] Gossip service initialized with VPS node {}",
            vps_node_id
        );
    }
}

pub fn build_router(state: AppState) -> Router<AppState> {
    let base = Router::new().with_state(state.clone());
    base
        .merge(crate::signing::routes::debug::debug_routes())
        // Core game session and move routes (These were missing from build_app_router)
        .merge(crate::signing::routes::main::routes())
        // Session-key signing endpoints — dual-accept guard: a valid per-user JWT
        // (preferred) or the legacy relay secret. See `require_relay_or_jwt`.
        .merge(crate::signing::routes::main::protected_routes().layer(
            axum::middleware::from_fn_with_state(
                state.clone(),
                crate::infrastructure::require_relay_or_jwt,
            ),
        ))
        // Feature-specific nested routes
        .nest("/api/auth", crate::signing::routes::auth::auth_routes())
        .nest("/api/actions", blinks::blinks_routes())
        .nest(
            "/api",
            crate::signing::routes::client_events::client_events_routes(),
        )
        .nest(
            "/api",
            crate::signing::routes::money_actions::money_action_routes().layer(
                axum::middleware::from_fn_with_state(
                    state.clone(),
                    crate::infrastructure::require_relay_or_jwt,
                ),
            ),
        )
        .nest("/api/rates", crate::signing::routes::rates::rates_routes())
        // Public RPC proxy for the distributed client — see routes::rpc_proxy docs.
        .nest(
            "/api",
            crate::signing::routes::rpc_proxy::rpc_proxy_routes(),
        )
        // Lobby handshake and region lookup; moves use a separate transport.
        .merge(p2p_relay::p2p_routes())
        .nest(
            "/identity",
            crate::signing::routes::identity::identity_routes(),
        )
        // WebSocket route for authentication sync
        .route("/ws/auth", get(handle_auth_websocket))
        // Session mutations require the shared transport guard and a wallet-bound JWT.
        // The read-only verify endpoint remains public.
        .nest(
            "/api/global-session",
            crate::signing::routes::global_session::global_session_public_routes(),
        )
        .nest(
            "/api/global-session",
            crate::signing::routes::global_session::global_session_protected_routes().layer(
                axum::middleware::from_fn_with_state(
                    state.clone(),
                    crate::infrastructure::require_relay_or_jwt,
                ),
            ),
        )
        // Anti-cheat verdict + player stats queries
        .nest(
            "/api",
            crate::signing::routes::anticheat::anticheat_routes(),
        )
        // Wallet balance (SOL + stablecoins via Helius, converted to local currency)
        .nest(
            "/api/wallet",
            crate::signing::routes::wallet::wallet_routes(),
        )
        // External ELO linking (Lichess)
        .nest(
            "/api",
            crate::signing::routes::external_elo::external_elo_routes(),
        )
        // Lichess OAuth 2.0 + PKCE flow (primary)
        .nest(
            "/api",
            crate::signing::routes::lichess_oauth::lichess_oauth_routes(),
        )
}
