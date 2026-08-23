//! XFChess Braid-HTTP server.
//!
//! Implements the [Braid-HTTP 209 subscribe protocol](https://braid.org/) for
//! streaming live tournament state (standings, pairings, roster, meta) to web
//! browsers without polling.
//!
//! # Mounting
//! ```no_run
//! # async fn example() {
//! use xfchess_braid_server::{ResourceHub, braid_router};
//! let hub = ResourceHub::new();
//! let router = braid_router(hub.clone());
//! // Mount on your axum App:
//! // let app = existing_app.nest("/braid", router);
//! # }
//! ```
//!
//! # Pushing updates
//! ```no_run
//! # use xfchess_braid_server::{ResourceHub, bridge};
//! # let hub = ResourceHub::new();
//! // After TournamentStore registers a new player:
//! bridge::push_roster(&hub, 42, &["wallet1".into(), "wallet2".into()]);
//! ```
//!
//! # License / Attribution
//! See `ATTRIBUTION.md`.

pub mod bridge;
pub mod hub;
pub mod resource;

pub use hub::{GossipSink, ResourceHub};
pub use resource::{AppendLog, PatchedDoc};

use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Build the Axum router for all `/braid/*` paths.
///
/// Mount this on your existing app with `.nest("/braid", braid_router(hub))`.
pub fn braid_router(hub: ResourceHub) -> Router {
    let hub = Arc::new(hub);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers(Any);

    Router::new()
        // axum 0.8 wildcard syntax. This read `/*res` (axum 0.7) until the
        // router was first mounted — `Router::route` *panics* on the old form,
        // so simply building this router would have taken the server down at
        // startup. Nothing caught it because nothing called `braid_router`.
        .route("/{*res}", get(resource::subscribe::get_resource))
        .layer(cors)
        .with_state(hub)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Building the router must not panic, and the wildcard must actually
    /// match a nested resource path. `Router::route` validates path syntax
    /// eagerly, so a wrong wildcard form fails here rather than in production.
    #[tokio::test]
    async fn the_router_serves_a_nested_resource_path() {
        let hub = ResourceHub::new();
        hub.ensure_tournament(42);
        bridge::push_standings(
            &hub,
            42,
            json!([{ "player_id": "a", "score": 1.0, "rank": 1 }]),
        );

        let server = axum_test::TestServer::new(braid_router(hub));

        // A plain GET (no Subscribe header) is an ordinary 200 snapshot.
        let response = server.get("/tournament/42/standings").await;
        response.assert_status_ok();
        response.assert_json(&json!([{ "player_id": "a", "score": 1.0, "rank": 1 }]));
    }

    /// An unregistered resource is a 404, not a hang or a panic.
    #[tokio::test]
    async fn an_unknown_resource_is_not_found() {
        let server = axum_test::TestServer::new(braid_router(ResourceHub::new()));
        server
            .get("/tournament/999/standings")
            .await
            .assert_status_not_found();
    }
}
