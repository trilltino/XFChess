//! Router building and merging for the XFChess backend.
//!
//! This module centralizes all router construction logic, combining
//! signing, tournament, and matchmaking routers into a single application router.

use axum::{middleware, Router};
use crate::signing::{AppState, build_router};
use crate::signing::storage::tournament::TournamentStore;
use crate::signing::routes::tournament as tournament_routes;
use crate::signing::routes::pdf_mailer::pdf_mailer_routes;
use crate::infrastructure::auth_middleware::require_api_key;

/// Builds the complete application router by merging all sub-routers.
///
/// # Arguments
/// * `signing_state` - The shared application state for signing routes
/// * `tournament_store` - The tournament store for tournament routes
///
/// # Returns
/// A merged Axum Router with all route handlers registered
pub fn build_app_router(
    signing_state: AppState,
    tournament_store: TournamentStore,
) -> Router {
    // Build signing router
    let signing_router = build_router(signing_state.clone());

    // Build tournament router with auth middleware on admin routes
    // Nest routes under path prefixes to avoid conflicts
    let tournament_router = Router::new()
        .nest("/tournaments", tournament_routes::tournaments_routes().with_state(tournament_store.clone()))
        .nest("/tournament", tournament_routes::tournament_routes().with_state(tournament_store.clone()))
        .nest("/admin/tournament",
            tournament_routes::admin_tournament_routes()
                .with_state(tournament_store.clone())
                .layer(middleware::from_fn(require_api_key))
        );

    // Build pdf mailer router (no auth required for signup)
    let pdf_router = pdf_mailer_routes();

    // Merge all routers
    signing_router
        .merge(tournament_router)
        .merge(pdf_router)
}
