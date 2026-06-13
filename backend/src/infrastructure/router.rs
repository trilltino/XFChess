//! Router building and merging for the XFChess backend.
//!
//! This module centralizes all router construction logic, combining
//! signing, tournament, and matchmaking routers into a single application router.

use axum::{extract::State, middleware, Router};
use crate::signing::{AppState, build_router};
use crate::signing::swiss::handlers::{swiss_admin_routes, swiss_read_routes};
use crate::signing::routes::tournament as tournament_routes;
use crate::signing::routes::matchmaking::matchmaking_routes;
use crate::signing::routes::pdf_mailer::pdf_mailer_routes;
use crate::signing::routes::kyc::kyc_routes;
use crate::signing::routes::history::history_routes;
use crate::signing::routes::dispute::{dispute_routes, admin_dispute_routes};
use crate::signing::routes::archive::archive_routes;
use crate::signing::routes::admin::admin_routes;
use crate::signing::routes::chat::routes as chat_routes;
use crate::signing::social::routes::social_routes;
use crate::infrastructure::auth_middleware::require_api_key;

/// Builds the complete application router by merging all sub-routers.
///
/// # Arguments
/// * `signing_state` - The shared application state for all routes
///
/// # Returns
/// A merged Axum Router with all route handlers registered
pub fn build_app_router(
    signing_state: AppState,
) -> Router<AppState> {
    // Build signing router (includes tournament routes via build_router)
    let signing_router = build_router(signing_state.clone());

    // Base router with AppState so all nested/merged routers share the same type
    let base = Router::new().with_state(signing_state.clone());

    // Build tournament router with auth middleware on admin routes
    // Note: tournament routes now use AppState directly
    let tournament_router = base
        .clone()
        .nest("/tournaments", tournament_routes::tournaments_routes())
        .nest("/tournament", tournament_routes::tournament_routes())
        .nest("/tournament", swiss_read_routes())
        .nest("/admin/tournament", swiss_admin_routes()
            .layer(middleware::from_fn(require_api_key))
        )
        .nest("/admin/tournament",
            tournament_routes::admin_tournament_routes()
                .layer(middleware::from_fn(require_api_key))
        );

    // Build matchmaking router — state provided by parent .with_state()
    let matchmaking_router = base
        .clone()
        .nest("/matchmaking", matchmaking_routes());

    // Build pdf mailer router (no auth required for signup)
    let pdf_router = pdf_mailer_routes();

    // Build KYC / user-status router (needs AppState for vault_pool + store)
    let kyc_router = kyc_routes();

    // Build game history router
    let history_router = history_routes();

    // Build dispute router
    let dispute_router = base
        .clone()
        .nest("/dispute", dispute_routes())
        .nest("/admin/dispute",
            admin_dispute_routes()
                .layer(middleware::from_fn(require_api_key))
        );

    // Build metrics endpoint — core HTTP/RPC metrics plus the background-worker
    // and anti-cheat/linkage counters (settlement, prize distribution, blur,
    // think-time discards, Sybil linkage).
    let metrics_router = base
        .clone()
        .route("/metrics", axum::routing::get(|State(app_state): State<AppState>| async move {
            let mut out = app_state.metrics.export_prometheus_format();
            out.push('\n');
            out.push_str(&crate::telemetry::worker_metrics::render_prometheus());
            out
        }));

    // Social (friends, presence, lobby invites)
    let social_router = social_routes(signing_state.invite_store.clone())
        .with_state(signing_state.clone());

    // Merge all routers and add CORS
    signing_router
        .merge(tournament_router)
        .merge(matchmaking_router)
        .merge(pdf_router)
        .merge(kyc_router)
        .merge(history_router)
        .merge(dispute_router)
        .merge(metrics_router)
        .merge(archive_routes().with_state(signing_state.clone()).layer(middleware::from_fn(require_api_key)))
        .merge(admin_routes().with_state(signing_state.clone()).layer(middleware::from_fn(require_api_key)))
        .merge(chat_routes().with_state(signing_state.clone()))
        .merge(social_router)
        .layer(
            tower_http::cors::CorsLayer::permissive()
        )
        .layer(tower_http::trace::TraceLayer::new_for_http())
}
