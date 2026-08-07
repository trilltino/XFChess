//! HTTP 209 subscribe handler.
//!
//! `GET /braid/<path>` with a subscribe header (see
//! [`braid_http::server::wants_subscribe`] for the spellings accepted) returns
//! status **209**, writes the current snapshot immediately, then holds the
//! connection open and streams each subsequent update until the client goes
//! away. A heartbeat every [`HEARTBEAT_SECS`] keeps proxies from closing an idle
//! stream, and lets the client tell a quiet resource from a dead connection.
//!
//! Without a subscribe header it is an ordinary `GET`: current state, status 200.

use crate::resource::protocol::{format_chunk, format_heartbeat};
use crate::ResourceHub;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use braid_http::server::SubscriptionResponse;
use bytes::Bytes;
use futures::StreamExt;
use std::sync::Arc;
use tokio::time::interval;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, warn};

/// How often an idle subscription is proven alive.
///
/// The client derives its liveness deadline from the `Heartbeats` header built
/// out of this, so raising it also slows how fast a dead stream is noticed.
const HEARTBEAT_SECS: u64 = 20;

/// GET handler for all Braid resources.
///
/// Path parameter `res` contains everything after `/braid/` (e.g.
/// `tournament/42/standings`).
pub async fn get_resource(
    State(hub): State<Arc<ResourceHub>>,
    Path(res): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !braid_http::server::wants_subscribe(|name| headers.get(name).and_then(|v| v.to_str().ok()))
    {
        return match hub.current_json(&res).await {
            Some(val) => (StatusCode::OK, Json(val)).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        };
    }
    subscribe_stream(hub, res).await
}

/// Build a 209 streaming response for the resource at `path`.
async fn subscribe_stream(hub: Arc<ResourceHub>, path: String) -> Response {
    let Some((snapshot, rx)) = hub.subscribe(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let braid = SubscriptionResponse::with_heartbeat_secs(HEARTBEAT_SECS);
    let snapshot_chunk = format_chunk(&snapshot);
    let heartbeat = format_heartbeat();
    let rx_stream = BroadcastStream::new(rx);
    let interval_duration = braid.heartbeat_interval;

    let stream = async_stream::stream! {
        debug!("[braid] subscriber connected to {}", path);

        yield Ok::<Bytes, String>(snapshot_chunk);

        let mut ticker = interval(interval_duration);
        ticker.tick().await; // consume the immediate first tick

        tokio::pin!(rx_stream);
        loop {
            tokio::select! {
                maybe_update = rx_stream.next() => {
                    match maybe_update {
                        Some(Ok(update)) => yield Ok(format_chunk(&update)),
                        // A lagging subscriber has already missed updates. Keep
                        // the stream open: later updates still arrive, and a
                        // reconnect replays from the snapshot.
                        Some(Err(e)) => warn!("[braid] broadcast lag on {}: {}", path, e),
                        None => {
                            debug!("[braid] channel closed for {}", path);
                            break;
                        }
                    }
                }
                _ = ticker.tick() => yield Ok(heartbeat.clone()),
            }
        }
        debug!("[braid] subscriber disconnected from {}", path);
    };

    let mut builder = Response::builder().status(braid.status);
    for (name, value) in braid.headers() {
        builder = builder.header(name, value);
    }
    builder
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
