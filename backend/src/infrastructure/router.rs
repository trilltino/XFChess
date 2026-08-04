//! Router building and merging for the XFChess backend.
//!
//! This module centralizes all router construction logic, combining
//! signing, tournament, and matchmaking routers into a single application router.

use crate::infrastructure::auth_middleware::require_api_key;
use crate::signing::routes::admin::admin_routes;
use crate::signing::routes::archive::archive_routes;
use crate::signing::routes::casual_games::casual_games_routes;
use crate::signing::routes::dispute::{admin_dispute_routes, dispute_routes};
use crate::signing::routes::game_log::routes as game_log_routes;
use crate::signing::routes::history::history_routes;
use crate::signing::routes::kyc::kyc_routes;
use crate::signing::routes::mailer::mailer_routes;
use crate::signing::routes::matchmaking::matchmaking_routes;
use crate::signing::routes::puzzle::{puzzle_admin_routes, puzzle_routes};
use crate::signing::routes::tournament as tournament_routes;
use crate::signing::social::routes::social_routes;
use crate::signing::swiss::handlers::{swiss_admin_routes, swiss_read_routes};
use crate::signing::{build_router, AppState};
use axum::{extract::State, middleware, Router};

/// Builds the complete application router by merging all sub-routers.
///
/// # Arguments
/// * `signing_state` - The shared application state for all routes
///
/// # Returns
/// A merged Axum Router with all route handlers registered
pub fn build_app_router(signing_state: AppState) -> Router<AppState> {
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
        .nest(
            "/admin/tournament",
            swiss_admin_routes().layer(middleware::from_fn(require_api_key)),
        )
        .nest(
            "/admin/tournament",
            tournament_routes::admin_tournament_routes()
                .layer(middleware::from_fn(require_api_key)),
        );

    // Build matchmaking router — state provided by parent .with_state()
    let matchmaking_router = base.clone().nest("/matchmaking", matchmaking_routes());

    // Build mailer router (no auth required for signup)
    let mail_router = mailer_routes();

    // Build KYC / user-status router (needs AppState for vault_pool + store)
    let kyc_router = kyc_routes();

    // Build casual (off-chain) game recording router
    let casual_games_router = casual_games_routes();

    // Build game history router
    let history_router = history_routes();

    // Build dispute router
    let dispute_router = base.clone().nest("/dispute", dispute_routes()).nest(
        "/admin/dispute",
        admin_dispute_routes().layer(middleware::from_fn(require_api_key)),
    );

    // Build metrics endpoint — health/version gauges, core HTTP/RPC/transaction
    // metrics, the background-worker and anti-cheat/linkage counters
    // (settlement, prize distribution, think-time discards, Sybil linkage),
    // and live presence gauges.
    let metrics_router = base.clone().route(
        "/metrics",
        axum::routing::get(|State(app_state): State<AppState>| async move {
            let mut out = format!(
                "# HELP xfchess_health Health status (1 = healthy, 0 = unhealthy)\n\
                 # TYPE xfchess_health gauge\n\
                 xfchess_health 1\n\
                 # HELP xfchess_version Version info\n\
                 # TYPE xfchess_version gauge\n\
                 xfchess_version{{version=\"{}\"}} 1\n",
                env!("CARGO_PKG_VERSION")
            );
            out.push('\n');
            out.push_str(&app_state.metrics.export_prometheus_format());
            out.push('\n');
            out.push_str(&crate::telemetry::worker_metrics::render_prometheus());
            out.push_str(&format!(
                "\n# HELP xfchess_presence_online_gauge Players currently in a game (bot, local, P2P, or Solana)\n\
                 # TYPE xfchess_presence_online_gauge gauge\n\
                 xfchess_presence_online_gauge {}\n\
                 # HELP xfchess_presence_games_in_progress_gauge Distinct real multiplayer games in progress\n\
                 # TYPE xfchess_presence_games_in_progress_gauge gauge\n\
                 xfchess_presence_games_in_progress_gauge {}\n",
                app_state.presence.count_in_game(),
                app_state.presence.count_games_in_progress(),
            ));
            out
        }),
    );

    // Social (friends, presence, lobby invites)
    let social_router =
        social_routes(signing_state.invite_store.clone()).with_state(signing_state.clone());

    // Merge all routers and add CORS
    signing_router
        .merge(tournament_router)
        .merge(matchmaking_router)
        .nest("/api", mail_router)
        .merge(kyc_router)
        .merge(casual_games_router)
        .merge(history_router)
        .merge(puzzle_routes())
        .merge(dispute_router)
        .merge(metrics_router)
        .merge(
            archive_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn(require_api_key)),
        )
        .merge(
            admin_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn(require_api_key)),
        )
        .merge(
            puzzle_admin_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn(require_api_key)),
        )
        .merge(game_log_routes().with_state(signing_state.clone()))
        .merge(social_router)
        // Records http_requests_total / http_request_duration_ms for every
        // request. This was defined but never registered anywhere — those
        // metrics were silently always zero until this layer was added.
        .layer(middleware::from_fn_with_state(
            signing_state.clone(),
            crate::telemetry::telemetry_middleware,
        ))
        .layer(cors_layer())
        // Correlation IDs: accept an inbound x-request-id or mint a UUID, include it in
        // the request span (so every log line within the request carries it), and echo
        // it on the response so clients/support can quote it.
        .layer(tower_http::propagate_header::PropagateHeaderLayer::new(
            axum::http::HeaderName::from_static("x-request-id"),
        ))
        .layer(
            tower_http::trace::TraceLayer::new_for_http().make_span_with(
                |request: &axum::http::Request<_>| {
                    let request_id = request
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("-");
                    tracing::info_span!(
                        "http",
                        method = %request.method(),
                        uri = %request.uri().path(),
                        request_id = %request_id,
                    )
                },
            ),
        )
        .layer(tower_http::request_id::SetRequestIdLayer::x_request_id(
            tower_http::request_id::MakeRequestUuid,
        ))
}

/// CORS layer built from the `ALLOWED_ORIGINS` env var (comma-separated list of
/// origins, e.g. `https://xfchess.com,https://www.xfchess.com`).
///
/// If `ALLOWED_ORIGINS` is unset/empty we fall back to permissive (any origin) —
/// convenient for local dev, but **ALLOWED_ORIGINS must be set in production**.
fn cors_layer() -> tower_http::cors::CorsLayer {
    use axum::http::{header, HeaderValue, Method};
    use tower_http::cors::{AllowOrigin, CorsLayer};

    let origins: Vec<HeaderValue> = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect();

    let base = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            // Admin panel (tournament-admin webview) authenticates with X-API-Key.
            header::HeaderName::from_static("x-api-key"),
        ]);

    if origins.is_empty() {
        tracing::warn!(
            "[cors] ALLOWED_ORIGINS not set — allowing any origin (dev only; set it in production)"
        );
        base.allow_origin(AllowOrigin::any())
    } else {
        tracing::info!("[cors] restricting origins to: {:?}", origins);
        base.allow_origin(AllowOrigin::list(origins))
    }
}
