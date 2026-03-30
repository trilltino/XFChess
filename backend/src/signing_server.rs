use backend::signing::{AppState, SigningConfig, build_router};
use tracing_subscriber::EnvFilter;
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = SigningConfig::from_env();
    let port = config.port;

    // SQLite pool for session persistence
    let pool = SqlitePool::connect("sqlite://sessions.db?mode=rwc").await?;
    tracing::info!("[signing-server] SQLite pool connected");

    let state = AppState::new(config, pool.clone());
    state.store.init().await?;
    tracing::info!("[signing-server] Sessions table initialized");

    let app = build_router(state);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("[signing-server] Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind port");
    axum::serve(listener, app).await.expect("server error");
    Ok(())
}
