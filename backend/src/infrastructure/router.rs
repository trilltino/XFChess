use crate::infrastructure::auth_middleware::{
    persist_admin_request, require_api_key, require_relay_or_jwt,
};
use crate::signing::routes::admin::admin_routes;
use crate::signing::routes::archive::archive_routes;
use crate::signing::routes::casual_games::casual_games_routes;
use crate::signing::routes::dispute::{
    admin_dispute_routes, dispute_notify_routes, dispute_routes,
};
use crate::signing::routes::game_log::routes as game_log_routes;
use crate::signing::routes::history::history_routes;
use crate::signing::routes::kyc::kyc_routes;
use crate::signing::routes::mailer::mailer_routes;
use crate::signing::routes::matchmaking::matchmaking_routes;
use crate::signing::routes::offline_tournament;
use crate::signing::routes::puzzle::{puzzle_admin_routes, puzzle_routes};
use crate::signing::routes::spectate::routes as spectate_routes;
use crate::signing::routes::tournament as tournament_routes;
use crate::signing::social::routes::social_routes;
use crate::signing::swiss::handlers::{swiss_admin_routes, swiss_read_routes};
use crate::signing::{build_router, AppState};
use axum::{extract::State, middleware, response::IntoResponse, Router};

const ERROR_BODY_SCRUB_LIMIT: usize = 64 * 1024;

/// Strips URLs (and the RPC provider tokens in their paths) from 4xx/5xx
/// bodies. Handlers format upstream errors straight into responses, and
/// reqwest's Display includes the full request URL.
async fn scrub_error_bodies(
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let response = next.run(request).await;
    if !(response.status().is_client_error() || response.status().is_server_error()) {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, ERROR_BODY_SCRUB_LIMIT).await else {
        return (parts.status, "error body unavailable").into_response();
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return axum::response::Response::from_parts(parts, axum::body::Body::from(bytes));
    };
    let scrubbed = crate::signing::solana::redact_urls_in(text);
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    axum::response::Response::from_parts(parts, axum::body::Body::from(scrubbed))
}

pub fn build_app_router(signing_state: AppState) -> Router<AppState> {
    let signing_router = build_router(signing_state.clone());

    let base = Router::new().with_state(signing_state.clone());

    let protected_tournament_join_routes = tournament_routes::tournament_join_routes().layer(
        middleware::from_fn_with_state(signing_state.clone(), require_relay_or_jwt),
    );
    let tournament_router = base
        .clone()
        // Mount API aliases under /api to avoid collisions with frontend tournament
        // pages served by the nginx SPA fallback.
        .nest("/api/tournaments", tournament_routes::tournaments_routes())
        .nest("/tournaments", tournament_routes::tournaments_routes())
        .nest("/api/tournament", tournament_routes::tournament_routes())
        .nest("/tournament", tournament_routes::tournament_routes())
        .nest("/api/tournament", protected_tournament_join_routes.clone())
        .nest("/tournament", protected_tournament_join_routes)
        .nest("/api/tournament", swiss_read_routes())
        .nest("/tournament", swiss_read_routes())
        .nest(
            "/admin/tournament",
            swiss_admin_routes()
                .layer(middleware::from_fn_with_state(
                    signing_state.clone(),
                    persist_admin_request,
                ))
                .layer(middleware::from_fn(require_api_key)),
        )
        .nest(
            "/admin/tournament",
            tournament_routes::admin_tournament_routes()
                .layer(middleware::from_fn_with_state(
                    signing_state.clone(),
                    persist_admin_request,
                ))
                .layer(middleware::from_fn(require_api_key)),
        );

    let matchmaking_router = base.clone().nest("/matchmaking", matchmaking_routes());

    let mail_router = mailer_routes();

    let kyc_router = kyc_routes();

    let casual_games_router = casual_games_routes();

    let history_router = history_routes();

    let offline_tournament_router = base
        .clone()
        .nest(
            "/offline-tournaments",
            offline_tournament::public_routes().with_state(signing_state.clone()),
        )
        .nest(
            "/admin/offline-tournaments",
            offline_tournament::admin_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn_with_state(
                    signing_state.clone(),
                    persist_admin_request,
                ))
                .layer(middleware::from_fn(require_api_key)),
        );

    let dispute_router = base
        .clone()
        .nest("/dispute", dispute_routes())
        .nest(
            "/dispute",
            dispute_notify_routes().layer(middleware::from_fn_with_state(
                signing_state.clone(),
                require_relay_or_jwt,
            )),
        )
        .nest(
            "/admin/dispute",
            admin_dispute_routes()
                .layer(middleware::from_fn_with_state(
                    signing_state.clone(),
                    persist_admin_request,
                ))
                .layer(middleware::from_fn(require_api_key)),
        );

    let metrics_router = base.route(
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
            out.push_str(&crate::telemetry::resources::render_prometheus());
            out.push_str(&crate::telemetry::rpc::render_prometheus());
            let pool = app_state.store.pool();
            out.push_str(&format!(
                "# TYPE xfchess_session_db_connections gauge\nxfchess_session_db_connections {}\n\
                 # TYPE xfchess_session_db_idle_connections gauge\nxfchess_session_db_idle_connections {}\n",
                pool.size(), pool.num_idle()));
            let filename = pool.connect_options().get_filename().to_path_buf();
            if let Ok(metadata) = std::fs::metadata(&filename) {
                let wal = std::path::PathBuf::from(format!("{}-wal", filename.display()));
                let bytes = metadata.len().saturating_add(std::fs::metadata(wal).map(|m| m.len()).unwrap_or(0));
                out.push_str(&format!("# TYPE xfchess_session_database_bytes gauge\nxfchess_session_database_bytes {bytes}\n"));
            }
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
            ([(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], out)
        }),
    );

    let social_router =
        social_routes(signing_state.invite_store.clone()).with_state(signing_state.clone());

    let router = signing_router
        .merge(tournament_router)
        .merge(matchmaking_router)
        .nest("/api", mail_router)
        .merge(kyc_router)
        .merge(casual_games_router)
        .merge(history_router)
        .merge(offline_tournament_router)
        .merge(puzzle_routes())
        .merge(dispute_router)
        .merge(metrics_router)
        .merge(
            archive_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn_with_state(
                    signing_state.clone(),
                    persist_admin_request,
                ))
                .layer(middleware::from_fn(require_api_key)),
        )
        .merge(
            admin_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn_with_state(
                    signing_state.clone(),
                    persist_admin_request,
                ))
                .layer(middleware::from_fn(require_api_key)),
        )
        .nest(
            "/api/actions",
            crate::signing::blinks::blinks_admin_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn_with_state(
                    signing_state.clone(),
                    persist_admin_request,
                ))
                .layer(middleware::from_fn(require_api_key)),
        )
        .merge(
            puzzle_admin_routes()
                .with_state(signing_state.clone())
                .layer(middleware::from_fn_with_state(
                    signing_state.clone(),
                    persist_admin_request,
                ))
                .layer(middleware::from_fn(require_api_key)),
        )
        .merge(game_log_routes().with_state(signing_state.clone()))
        .merge(spectate_routes().with_state(signing_state.clone()))
        // Use nest_service because braid_router already has its own applied state.
        .nest_service(
            "/braid",
            xfchess_braid_server::braid_router((*signing_state.braid_hub).clone()),
        )
        .merge(social_router)
        .layer(middleware::from_fn_with_state(
            signing_state.clone(),
            crate::telemetry::telemetry_middleware,
        ))
        .layer(middleware::from_fn(scrub_error_bodies))
        // Cap caller-supplied transaction and JSON payloads at 1 MiB to bound allocation.
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .layer(cors_layer(&signing_state.config.allowed_origins))
        // Accept or generate a request ID, attach it to the tracing span, and echo it to clients.
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
        ));

    #[cfg(feature = "coverage")]
    let router = router.layer(middleware::from_fn(crate::coverage::record_hit));
    router
}

fn cors_layer(allowed_origins: &[String]) -> tower_http::cors::CorsLayer {
    use axum::http::{header, HeaderValue, Method};
    use tower_http::cors::{AllowOrigin, CorsLayer};

    let origins: Vec<HeaderValue> = allowed_origins
        .iter()
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
