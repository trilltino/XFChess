use backend::signing::{AppState, SigningConfig};
use backend::signing::storage::tournament::TournamentStore;
use backend::infrastructure::{initialize_pools, run_migrations, build_app_router, spawn_background_tasks};

// TournamentTrigger imported via signing module
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use tracing::info;
use axum::http::{HeaderValue, Method};
use tower_http::cors::CorsLayer;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = SigningConfig::from_env();
    let port = config.port;

    // -- Initialize database pools ----------------------------------------
    let session_db = std::env::var("SESSION_DB_URL")
        .unwrap_or_else(|_| "sqlite://sessions.db?mode=rwc".into());
    let vault_db = std::env::var("VAULT_DB_URL")
        .unwrap_or_else(|_| "sqlite://vault.db?mode=rwc".into());
    let pools = initialize_pools(&session_db, &vault_db).await?;
    info!("[signing-server] Database pools initialized");

    // -- Run database migrations -------------------------------------------
    run_migrations(&pools).await?;
    info!("[signing-server] Database migrations completed");

    // -- Initialize application state -------------------------------------
    let session_store = backend::signing::storage::SessionStore::new(pools.session_pool.clone());
    session_store.init().await?;
    info!("[signing-server] Session store initialized");

    let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;
    info!("[signing-server] Tournament store initialized");

    let mut state = AppState::new(config.clone(), pools.session_pool.clone(), pools.vault_pool.clone(), Arc::new(tournament_store.clone()));

    // -- Spawn background tasks (must be before building router to get trigger sender) -----------------------------------------------
    let tournament_trigger = spawn_background_tasks(state.clone(), config);
    state.tournament_trigger = Some(tournament_trigger);
    info!("[signing-server] Background tasks spawned with tournament scheduler");

    // -- Build application router -------------------------------------------
    let app = build_app_router(state.clone());
    
    // Add CORS layer — restrict to configured origins in production
    let allowed_origins: Vec<HeaderValue> = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173,http://localhost:3000".into())
        .split(',')
        .filter_map(|o| o.trim().parse::<HeaderValue>().ok())
        .collect();
    let cors = if allowed_origins.is_empty() {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
            .allow_origin(allowed_origins)
            .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .allow_headers(tower_http::cors::Any)
    };
    let app = app.layer(cors);
    
    info!("[signing-server] Application router built");

    // -- Bind and serve via HTTP -------------------------------------------
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("============================================================");
    info!(" XFCHESS BACKEND IS NOW ONLINE");
    info!(" Listening on: http://127.0.0.1:{}", port);
    info!(" Admin Key: Configured in .env");
    info!("============================================================");

    // -- Serve HTTP -------------------------------------------------------
    axum::serve(listener, app).await?;

    Ok(())
}

