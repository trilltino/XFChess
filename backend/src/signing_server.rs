use backend::signing::{AppState, SigningConfig};
use backend::signing::storage::tournament::TournamentStore;
use backend::signing::storage::SessionStore;
use backend::infrastructure::{initialize_pools, run_migrations, spawn_background_tasks};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use tracing::info;

const PID_FILE: &str = ".backend.pid";

fn write_pid_file() {
    let pid = std::process::id();
    if let Err(e) = std::fs::write(PID_FILE, pid.to_string()) {
        tracing::warn!("[signing-server] Could not write PID file: {}", e);
    }
}

fn remove_pid_file() {
    let _ = std::fs::remove_file(PID_FILE);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = SigningConfig::from_env();
    let port = config.port;

    // ── Write PID so the launcher can kill exactly this process on restart ──
    write_pid_file();

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

    // ── Initialize social tables (friends, contacts) ─────────────────────────
    if let Err(e) = state.friends.init().await {
        tracing::warn!("[signing-server] Failed to init friends tables: {}", e);
    }
    info!("[signing-server] Social tables initialized");

    // ── Build application router ───────────────────────────────────────────
    let app = backend::infrastructure::build_app_router(state.clone());
    info!("[signing-server] Application router built");

    // ── Spawn Braid-Iroh P2P node (ephemeral UDP port, no conflict with HTTP) ─
    info!("[signing-server] Spawning Braid-Iroh Node (ephemeral port)");
    let (_braid_state, _rx) = braid_iroh::spawn_node(
        "xfchess-vps",
        None,
        None,
        braid_iroh::DiscoveryConfig::Real,
    ).await.map_err(|e| anyhow::anyhow!("failed to spawn braid-iroh node: {}", e))?;

    // ── Spawn background tasks ───────────────────────────────────────────────
    let (tournament_trigger, ac_queue) = spawn_background_tasks(state.clone(), config);
    state.tournament_trigger = Some(tournament_trigger);
    state.anticheat_queue = Some(ac_queue);
    info!("[signing-server] Background tasks spawned");

    // ── Start HTTP Server ──────────────────────────────────────────────────
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await
        .map_err(|e| anyhow::anyhow!("Failed to bind TCP listener on port {}: {}", port, e))?;
    info!("[signing-server] Listening for HTTP traffic on port {}", port);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app.with_state(state.clone())).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    // ── Wait for shutdown signal ───────────────────────────────────────────
    tokio::signal::ctrl_c().await.map_err(|e| anyhow::anyhow!("failed to wait for sigint: {}", e))?;
    info!("[signing-server] Shutting down");
    remove_pid_file();

    Ok(())
}
