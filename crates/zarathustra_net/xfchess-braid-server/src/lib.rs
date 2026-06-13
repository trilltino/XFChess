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

pub use hub::ResourceHub;
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
        .route("/*res", get(resource::subscribe::get_resource))
        .layer(cors)
        .with_state(hub)
}
