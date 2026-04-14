use backend::signing::{AppState, SigningConfig};
use backend::signing::storage::tournament::TournamentStore;
use backend::signing::storage::SessionStore;
use backend::infrastructure::{initialize_pools, run_migrations, build_app_router, spawn_background_tasks};
use tracing_subscriber::EnvFilter;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = SigningConfig::from_env();
    let port = config.port;

    // ── Initialize database pools ────────────────────────────────────────
    let pools = initialize_pools("sqlite://sessions.db?mode=rwc", "sqlite://vault.db?mode=rwc").await?;
    info!("[signing-server] Database pools initialized");

    // ── Run database migrations ───────────────────────────────────────────
    run_migrations(&pools).await?;
    info!("[signing-server] Database migrations completed");

    // ── Initialize application state ─────────────────────────────────────
    let session_store = SessionStore::new(pools.session_pool.clone());
    session_store.init().await?;
    info!("[signing-server] Session store initialized");

    let state = AppState::new(config.clone(), pools.session_pool.clone(), pools.vault_pool.clone());
    let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;
    info!("[signing-server] Tournament store initialized");

    // ── Build application router ───────────────────────────────────────────
    let app = build_app_router(state.clone(), tournament_store);
    info!("[signing-server] Application router built");

    // ── Bind and serve via Iroh P2P ───────────────────────────────────────
    info!("[signing-server] Spawning Braid-Iroh Node on port {port}");

    let (_braid_state, _rx) = braid_iroh::spawn_node(
        "xfchess-vps",
        Some(port),
        None,
        braid_iroh::DiscoveryConfig::Real,
        Some(pools.session_pool.clone()),
        Some(app),
    ).await.expect("failed to spawn braid-iroh node");

    // ── Spawn background tasks ───────────────────────────────────────────────
    spawn_background_tasks(state, config);
    info!("[signing-server] Background tasks spawned");

    // ── Wait for shutdown signal ───────────────────────────────────────────
    tokio::signal::ctrl_c().await.expect("failed to wait for sigint");
    info!("[signing-server] Shutting down");

    Ok(())
}
