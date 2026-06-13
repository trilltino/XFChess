//! HTTP 209 subscribe handler.
//!
//! A client sends `GET /braid/<path>` with a `Prefer: subscribe=keep-alive`
//! (or `Subscribe: keep-alive`) header. This handler:
//!
//! 1. Returns status **209** with `Content-Type: multipart/mixed; boundary=…`
//! 2. Immediately writes the current snapshot as the first multipart chunk.
//! 3. Keeps the connection open and streams subsequent patches via a
//!    [`tokio::broadcast`] receiver until the client disconnects.
//! 4. Sends an empty heartbeat every [`HEARTBEAT_SECS`] to prevent proxy
//!    timeouts.
//!
//! For plain GET (no Subscribe header) it returns the current state as JSON
//! with status 200.

use crate::resource::protocol::{format_chunk, format_heartbeat};
use crate::ResourceHub;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, warn};

const HEARTBEAT_SECS: u64 = 20;
const BRAID_BOUNDARY: &str = "xfchess-braid";

fn wants_subscribe(headers: &HeaderMap) -> bool {
    headers
        .get("Prefer")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("subscribe"))
        .unwrap_or(false)
        || headers
            .get("Subscribe")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("keep-alive"))
            .unwrap_or(false)
}

/// GET handler for all Braid resources.
///
/// Path parameter `res` contains everything after `/braid/` (e.g.
/// `tournament/42/standings`).
pub async fn get_resource(
    State(hub): State<Arc<ResourceHub>>,
    Path(res): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !wants_subscribe(&headers) {
        match hub.current_json(&res).await {
            Some(val) => (StatusCode::OK, Json(val)).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    } else {
        subscribe_stream(hub, res).await
    }
}

/// Build a 209 streaming response for the resource at `path`.
async fn subscribe_stream(hub: Arc<ResourceHub>, path: String) -> Response {
    let Some((snapshot, rx)) = hub.subscribe(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let boundary = BRAID_BOUNDARY;
    let ct = format!("multipart/mixed; boundary=\"{}\"", boundary);

    let snapshot_chunk = format_chunk(boundary, &snapshot);
    let hb_chunk = format_heartbeat(boundary);
    let rx_stream = BroadcastStream::new(rx);

    let stream = async_stream::stream! {
        debug!("[braid] subscriber connected to {}", path);

        // Initial snapshot
        yield Ok::<Bytes, String>(snapshot_chunk);

        let mut ticker = interval(Duration::from_secs(HEARTBEAT_SECS));
        ticker.tick().await; // consume immediate first tick

        tokio::pin!(rx_stream);
        loop {
            tokio::select! {
                maybe_update = rx_stream.next() => {
                    match maybe_update {
                        Some(Ok(update)) => {
                            yield Ok(format_chunk(boundary, &update));
                        }
                        Some(Err(e)) => {
                            warn!("[braid] broadcast lag on {}: {}", path, e);
                        }
                        None => {
                            debug!("[braid] channel closed for {}", path);
                            break;
                        }
                    }
                }
                _ = ticker.tick() => {
                    yield Ok(hb_chunk.clone());
                }
            }
        }
    };

    Response::builder()
        .status(209)
        .header("Content-Type", ct)
        .header("Subscribe", "keep-alive")
        .header("Cache-Control", "no-store")
        .body(Body::from_stream(stream))
        .unwrap()
}
