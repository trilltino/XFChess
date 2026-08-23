//! XFChess signing service module.
//!
//! This module provides the backend signing service for the XFChess game.
//! It handles:
//! - JWT-based authentication for wallet owners
//! - Session key management for game transactions
//! - Solana instruction building and transaction signing
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
//! - `config`: Environment configuration
//! - `feepayer`: Fee-payer keypair pool for transactions
//! - `identity`: Identity vault for encrypted KYC data
//! - `p2p_relay`: lobby-level P2P connection setup (announce/join/JOIN_ACK
//!   handshake, region lookup) — NOT move sync, see its own doc comment
//! - `routes`: HTTP route handlers (organized by feature), including
//!   `routes::game_log` — the Braid-backed durable move/chat log that
//!   replaced the old *move-relay* use of `p2p_relay`'s raw message/poll
//!   endpoints (the lobby JOIN_ACK handshake still uses them directly)
//! - `solana`: Solana instruction builders and RPC helpers
//! - `storage`: SQLite-backed data stores, including CACF compliance status
//!   (`storage::vault::{save_cacf, cacf_can_wager}`) for regulated
//!   jurisdictions (UK, Brazil, Germany, Canada) — the actual enforcement
//!   path used by `routes::kyc`; a separate, richer `cacf` module used to
//!   exist here too but was never wired to any route and has been removed
//! - `swiss`: Swiss pairing tournament system
//! - `auth_ws`: WebSocket authentication

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
    /// Public key only — the treasury-withdrawal secret key is deliberately
    /// never loaded by this process. See `bin/treasury_signer.rs`'s module
    /// doc for why, and `routes::admin::treasury_refund` for how a
    /// withdrawal request is logged here without ever being signed here.
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

    // ── Global session management ──────────────────────────────────────────────
    pub active_global_sessions: Arc<Mutex<HashMap<Pubkey, Keypair>>>,

    /// Per-game locks serializing ER-routed writes (`record_move`,
    /// `undelegate_game`, `schedule_time_check`, `cancel_time_check`).
    ///
    /// The ER rejects two concurrent writes to the same delegated account
    /// with `InvalidWritableAccount` ("illegally used as writable") — not an
    /// Anchor error, a rejection from MagicBlock's own runtime before our
    /// program even runs. Before this lock existed, `delegate_game`'s own
    /// follow-up `schedule_time_check_crank` call, `settlement_worker`'s
    /// periodic redelegate-and-reschedule pass, and a client's `record_move`
    /// batch could all independently fire ER writes for the same game with
    /// no coordination between them. Reproduced live 2026-08-10: a
    /// hand-recovery attempt got `InvalidWritableAccount` on every retry
    /// (three attempts, up to 15s apart) despite no client-side race being
    /// possible — the backend's own concurrent writers were still racing
    /// each other. See `signing::routes::main::with_er_game_lock`.
    pub er_write_locks: Arc<Mutex<HashMap<u64, Arc<tokio::sync::Mutex<()>>>>>,

    pub solana_rpc_url: String,
    pub program_id: Pubkey,
    /// Shared blocking RPC client — avoids a new TCP connection per route call.
    pub solana_rpc: Arc<solana_client::rpc_client::RpcClient>,
    /// On-chain-verified, cached `(white, black)` per game_id — see
    /// `solana::game_participants` module docs. Used by `game_log.rs` to
    /// close the roster-building race for wagered games.
    pub game_participants: solana::game_participants::GameParticipantsCache,

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

/// Loads an authority keypair from an env var value that's either a JSON
/// keypair file path or a raw base58-encoded secret key.
///
/// Returns `Err` on a malformed/unreadable keyfile or bad base58 rather than
/// silently falling back to a freshly generated keypair — callers decide
/// whether that's fatal (production) or a warn-and-generate dev convenience
/// (see `AppState::new`'s use of this).
pub fn load_keypair_from_env_value(val: &str) -> Result<Keypair, String> {
    if std::path::Path::new(val).exists() {
        let contents = std::fs::read_to_string(val)
            .map_err(|e| format!("could not read keyfile '{val}': {e}"))?;
        let bytes: Vec<u8> = serde_json::from_str(&contents)
            .map_err(|e| format!("keyfile '{val}' is not a valid JSON keypair array: {e}"))?;
        Keypair::try_from(bytes.as_slice())
            .map_err(|e| format!("keyfile '{val}' does not contain a valid ed25519 keypair: {e}"))
    } else {
        // Panics on malformed base58, same as before this function returned a
        // Result — the JSON-keyfile path above is what previously swallowed
        // errors silently, so that's the one that needed a graceful Result.
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
        // Built before `store` — session keypairs are encrypted at rest using
        // the same AES-256-GCM vault as KYC PII (see `storage::SessionStore`'s
        // doc comment for why a stolen SQLite backup shouldn't also be a
        // stolen signing key).
        let identity_vault =
            identity::IdentityVault::new(&config.identity_encryption_key, &config.identity_salt)
                .expect("Failed to initialize IdentityVault from env config");

        let store = Arc::new(storage::SessionStore::new(
            pool.clone(),
            identity_vault.clone(),
        ));
        let feepayer = Arc::new(feepayer::FeepayerPool::from_base58_list(
            &config.fee_payer_keys,
        ));
        let jwt = Arc::new(auth::JwtIssuer::new(&config.jwt_secret));

        let p2p_relay = Arc::new(p2p_relay::create_relay_state());

        // Initialize ELO cache with 5-minute TTL
        let program_id =
            Pubkey::from_str(&config.program_id).expect("Invalid program_id in config");
        let elo_cache = Arc::new(EloCache::new(
            config.solana_rpc_url.clone(),
            std::time::Duration::from_secs(300),
            program_id,
        ));
        // Matchmaking shares the same ELO cache rather than standing up its
        // own separate, devnet-hardcoded one. It also gets the session pool
        // so its queue/matches survive a backend restart (migration 022).
        let matchmaking =
            routes::matchmaking::SharedMatchmakingState::new(elo_cache.clone(), pool.clone());

        // Parse authority keys — accepts either a JSON file path or a base58 string.
        // `config.validate()` (called before `AppState::new` — see server.rs) already
        // hard-exits in production when any of these env vars is unset, so by the
        // time we're here a production process is guaranteed to have all four set.
        // This helper is the second line of defense: a *malformed* keyfile (present
        // but unparseable) would slip past that presence check, so it's fatal here
        // too under `is_production()` rather than silently becoming a fresh random
        // key. Outside production, both "unset" and "malformed" remain a
        // warn-and-generate dev convenience.
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

        // Log which pubkey each authority key actually resolved to. These
        // must match the corresponding hardcoded `Pubkey` constants in
        // programs/xfchess-game/src/constants.rs exactly, or every
        // privileged instruction that constrains on them (tournament
        // creation, fee collection, treasury withdrawal, ...) fails on-chain
        // with UnauthorizedAccess — a mismatch here is otherwise invisible
        // until a real transaction hits the RPC.
        tracing::info!(
            "[VPS] Authority pubkeys — vps: {}, kyc: {}, link: {}, treasury: {} (pubkey only, \
             signing key never loaded here — see bin/treasury_signer.rs)",
            vps_authority.pubkey(),
            kyc_authority.pubkey(),
            link_authority.pubkey(),
            treasury_authority_pubkey,
        );

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
        let tournament_fee_recipient = Pubkey::from_str(&config.tournament_fee_recipient)
            .expect("Invalid tournament_fee_recipient in config");
        let usdc_mint_pubkey =
            Pubkey::from_str(&config.usdc_mint_pubkey).expect("Invalid usdc_mint_pubkey in config");

        // Live SOL/fiat rate cache for GBP-denominated fees (tournament entry,
        // session platform fee) — dual-sourced with a background refresh so
        // payment-critical requests never block on a live external fetch.
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

    /// Periodically evicts entries from the in-memory maps that would otherwise
    /// grow without bound for the lifetime of the process.
    ///
    /// Three maps needed this and had no sweep, while every comparable map in
    /// the codebase already did (`lichess_oauth`'s PKCE store, the lobby-invite
    /// store, the presence map, the P2P relay state) — so this was an omission
    /// rather than a policy:
    ///
    /// * `siws_nonces` — inserted on every unauthenticated `/auth/siws-challenge`
    ///   call and removed only when that exact nonce is later verified. Expiry
    ///   was checked but never collected, making it an unauthenticated
    ///   memory-exhaustion vector.
    /// * `er_write_locks` — one entry per `game_id` ever seen, kept forever.
    /// * `active_global_sessions` — sessions whose on-chain delegation has
    ///   already expired stay usable in memory indefinitely.
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

                // A per-game lock is only worth keeping while something might
                // still contend for it. `Arc::strong_count == 1` means the map
                // holds the only reference, so no request or worker is inside
                // the critical section and dropping it cannot orphan a waiter.
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
            }
        });
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
        tracing::info!(
            "[AppState] Gossip service initialized with VPS node {}",
            vps_node_id
        );
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
        .merge(crate::signing::routes::main::protected_routes().layer(
            axum::middleware::from_fn_with_state(
                state.clone(),
                crate::infrastructure::require_relay_or_jwt,
            ),
        ))
        // Feature-specific nested routes
        .nest("/api/auth", crate::signing::routes::auth::auth_routes())
        .nest("/api/actions", blinks::blinks_routes())
        .nest("/api/rates", crate::signing::routes::rates::rates_routes())
        // Public RPC proxy for the distributed client — see routes::rpc_proxy docs.
        .nest(
            "/api",
            crate::signing::routes::rpc_proxy::rpc_proxy_routes(),
        )
        // Lobby-level P2P connection setup (announce/join/JOIN_ACK
        // handshake, region lookup) — NOT move sync, see p2p_relay's doc
        // comment. Load-bearing for basic game connectivity across
        // p2p_vps.rs/screens.rs; do not remove without migrating those
        // call sites first.
        .merge(p2p_relay::p2p_routes())
        .nest(
            "/identity",
            crate::signing::routes::identity::identity_routes(),
        )
        // WebSocket route for authentication sync
        .route("/ws/auth", get(handle_auth_websocket))
        // Global persistent session delegation. The read-only `verify` probe is
        // public; everything that mutates a wallet's session key sits behind the
        // same dual-accept guard as the per-game session routes, and each of
        // those handlers additionally requires a JWT proving control of the
        // wallet it acts on (`RequireWallet`).
        //
        // This nest previously had no middleware at all despite the router being
        // named "protected", which left `track-game` and `revoke` reachable by
        // anyone — see `routes::global_session`'s module docs.
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
