use backend::signing::{AppState, SigningConfig};
use backend::signing::storage::tournament::TournamentStore;
use backend::infrastructure::{initialize_pools, run_migrations, build_app_router, spawn_background_tasks};

// TournamentTrigger imported via signing module
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use tracing::info;
use axum::http::Method;
use tower_http::cors::{CorsLayer, Any};
use tokio::net::TcpListener;

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
    let session_store = backend::signing::storage::SessionStore::new(pools.session_pool.clone());
    session_store.init().await?;
    info!("[signing-server] Session store initialized");

    let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;
    info!("[signing-server] Tournament store initialized");

    let mut state = AppState::new(config.clone(), pools.session_pool.clone(), pools.vault_pool.clone(), Arc::new(tournament_store.clone()));

    // ── Spawn background tasks (must be before building router to get trigger sender) ───────────────────────────────────────────────
    let tournament_trigger = spawn_background_tasks(state.clone(), config);
    state.tournament_trigger = Some(tournament_trigger);
    info!("[signing-server] Background tasks spawned with tournament scheduler");

    // ── Build application router ───────────────────────────────────────────
    let app = build_app_router(state.clone());
    
    // Add CORS layer
    let app = app.layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any)
    );
    
    info!("[signing-server] Application router built");

    // ── Bind and serve via HTTP ───────────────────────────────────────────
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("[signing-server] HTTP server listening on port {}", port);

    // ── Serve HTTP ───────────────────────────────────────────────────────
    axum::serve(listener, app).await?;

    Ok(())
}
