//! Router building and merging for the XFChess backend.
//!
//! This module centralizes all router construction logic, combining
//! signing, tournament, and matchmaking routers into a single application router.

use axum::{middleware, Router};
use crate::signing::{AppState, build_router};
use crate::signing::swiss::handlers::swiss_routes;
use crate::signing::routes::tournament as tournament_routes;
use crate::signing::routes::matchmaking::matchmaking_routes;
use crate::signing::routes::pdf_mailer::pdf_mailer_routes;
use crate::signing::routes::kyc::kyc_routes;
use crate::signing::routes::history::history_routes;
use crate::signing::routes::dispute::{dispute_routes, admin_dispute_routes};
use crate::signing::routes::archive::archive_routes;
use crate::signing::routes::admin::admin_routes;
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
) -> Router {
    // Build signing router (includes tournament routes via build_router)
    let signing_router = build_router(signing_state.clone());

    // Build tournament router with auth middleware on admin routes
    // Note: tournament routes now use AppState directly
    let tournament_router = Router::new()
        .nest("/tournaments", tournament_routes::tournaments_routes().with_state(signing_state.clone()))
        .nest("/tournament", tournament_routes::tournament_routes().with_state(signing_state.clone()))
        .nest("/tournament", swiss_routes().with_state(signing_state.clone()))
        .nest("/admin/tournament",
            tournament_routes::admin_tournament_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn(require_api_key))
        );

    // Build matchmaking router — uses the single shared matchmaking state
    let matchmaking_router = Router::new()
        .nest("/matchmaking", matchmaking_routes(signing_state.matchmaking.clone()));
    // Note: matchmaking_routes provides its own state internally via with_state()

    // Build pdf mailer router (no auth required for signup)
    let pdf_router = pdf_mailer_routes();

    // Build KYC / user-status router (needs AppState for vault_pool + store)
    let kyc_router = kyc_routes().with_state(signing_state.clone());

    // Build game history router
    let history_router = history_routes().with_state(signing_state.clone());

    // Build dispute router
    let dispute_router = Router::new()
        .nest("/dispute", dispute_routes().with_state(signing_state.clone()))
        .nest("/admin/dispute",
            admin_dispute_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn(require_api_key))
        );

    // Build metrics endpoint
    let metrics_state = signing_state.clone();
    let metrics_router = Router::new()
        .route("/metrics", axum::routing::get(move || {
            let metrics = metrics_state.metrics.clone();
            async move {
                metrics.export_prometheus_format()
            }
        }));

    // Merge all routers and add CORS
    signing_router
        .merge(tournament_router)
        .merge(matchmaking_router)
        .merge(pdf_router)
        .merge(kyc_router)
        .merge(history_router)
        .merge(dispute_router)
        .merge(metrics_router)
        .nest("/", archive_routes().with_state(signing_state.clone()).layer(middleware::from_fn(require_api_key)))
        .nest("/", admin_routes().with_state(signing_state.clone()).layer(middleware::from_fn(require_api_key)))
        .layer(
            tower_http::cors::CorsLayer::permissive()
        )
        .layer(tower_http::trace::TraceLayer::new_for_http())
}
