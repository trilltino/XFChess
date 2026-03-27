pub mod auth;
pub mod config;
pub mod feepayer;
pub mod routes;
pub mod solana;
pub mod store;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub use auth::JwtIssuer;
pub use config::SigningConfig;
pub use feepayer::FeepayerPool;
pub use store::SessionStore;

/// Shared state injected into every route handler.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<SigningConfig>,
    pub store: SessionStore,
    pub feepayer: FeepayerPool,
    pub jwt: Arc<JwtIssuer>,
}

impl AppState {
    pub fn new(config: SigningConfig, pool: sqlx::SqlitePool) -> Self {
        let feepayer = FeepayerPool::from_base58_list(&config.fee_payer_keys);
        let jwt = Arc::new(JwtIssuer::new(&config.jwt_secret));
        let store = SessionStore::new(pool);
        Self {
            config: Arc::new(config),
            store,
            feepayer,
            jwt,
        }
    }
}

/// Build the Axum router for the signing service.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/auth/issue", post(routes::issue_jwt))
        .route("/session/create", post(routes::create_session))
        .route("/session/activate", post(routes::activate_session))
        .route("/session/sign", post(routes::sign_tx))
        .route("/session/status/{game_id}", get(routes::session_status))
        .route("/move/record", post(routes::record_move))
        .route("/game/undelegate", post(routes::undelegate_game))
        .route("/game/finalize", post(routes::finalize_game))
        .with_state(state)
}
