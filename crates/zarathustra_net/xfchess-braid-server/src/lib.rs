pub mod bridge;
pub mod hub;
pub mod resource;

pub use hub::{GossipSink, ResourceHub};
pub use resource::{AppendLog, PatchedDoc};

use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

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

    #[tokio::test]
    async fn an_unknown_resource_is_not_found() {
        let server = axum_test::TestServer::new(braid_router(ResourceHub::new()));
        server
            .get("/tournament/999/standings")
            .await
            .assert_status_not_found();
    }
}
