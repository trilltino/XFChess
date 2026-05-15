use backend::signing::{AppState, SigningConfig};
use backend::signing::storage::tournament::TournamentStore;
use backend::signing::storage::SessionStore;
use backend::infrastructure::{initialize_pools, run_migrations, spawn_background_tasks};
use std::sync::Arc;
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

    let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;
    info!("[signing-server] Tournament store initialized");

    let mut state = AppState::new(config.clone(), pools.session_pool.clone(), pools.vault_pool.clone(), Arc::new(tournament_store.clone()));

    // ── Build application router ───────────────────────────────────────────
    // Use the comprehensive app router from infrastructure to include all services
    let app = backend::infrastructure::build_app_router(state.clone());
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
    ).await.map_err(|e| anyhow::anyhow!("failed to spawn braid-iroh node: {}", e))?;

    // ── Spawn background tasks ───────────────────────────────────────────────
    let tournament_trigger = spawn_background_tasks(state.clone(), config);
    state.tournament_trigger = Some(tournament_trigger);
    info!("[signing-server] Background tasks spawned with tournament scheduler");

    // ── Wait for shutdown signal ───────────────────────────────────────────
    tokio::signal::ctrl_c().await.map_err(|e| anyhow::anyhow!("failed to wait for sigint: {}", e))?;
    info!("[signing-server] Shutting down");

    Ok(())
}
