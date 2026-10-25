use crate::{store::ResourceStore, subscription::SubscriptionManager};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use braid_http::{client::BraidHeaders, Update, Version};
use bytes::Bytes;
use iroh::EndpointId;
use iroh_h3_axum::{IrohAxum, RemoteId};
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Which authenticated QUIC endpoints may read or write resources over H3.
///
/// The default policy denies everyone. Holding an endpoint ID, a gossip topic
/// or a game URL never grants access; only an explicit grant does. This is
/// transport authority only: chess player, wallet and seat authority stay in
/// the application and backend.
#[derive(Clone, Debug, Default)]
pub struct AccessPolicy {
    readers: HashSet<EndpointId>,
    writers: HashSet<EndpointId>,
    public_read: bool,
}

impl AccessPolicy {
    pub fn deny_all() -> Self {
        Self::default()
    }

    /// Let `peer` read (subscribe to) resources.
    pub fn allow_read(&mut self, peer: EndpointId) {
        self.readers.insert(peer);
    }

    /// Let `peer` read and write resources.
    pub fn allow_write(&mut self, peer: EndpointId) {
        self.writers.insert(peer);
    }

    /// Open reads to every authenticated endpoint (spectators). Writes stay explicit.
    pub fn set_public_read(&mut self, public: bool) {
        self.public_read = public;
    }

    pub fn revoke(&mut self, peer: &EndpointId) {
        self.readers.remove(peer);
        self.writers.remove(peer);
    }

    pub fn can_read(&self, peer: &EndpointId) -> bool {
        self.public_read || self.readers.contains(peer) || self.writers.contains(peer)
    }

    pub fn can_write(&self, peer: &EndpointId) -> bool {
        self.writers.contains(peer)
    }
}

#[derive(Clone)]
pub(crate) struct BraidAppState {
    pub subscriptions: Arc<SubscriptionManager>,
    pub resources: Arc<ResourceStore>,
    /// This node. The loopback bridge acts as the node itself.
    pub local_id: EndpointId,
    pub access: Arc<RwLock<AccessPolicy>>,
    /// Bumped whenever the policy changes so open subscriptions re-check it.
    pub policy_changed: tokio::sync::watch::Sender<u64>,
}

impl BraidAppState {
    fn can_read(&self, peer: &EndpointId) -> bool {
        *peer == self.local_id || self.access.read().can_read(peer)
    }

    fn can_write(&self, peer: &EndpointId) -> bool {
        *peer == self.local_id || self.access.read().can_write(peer)
    }
}

pub(crate) fn build_protocol_handler(state: BraidAppState) -> IrohAxum {
    IrohAxum::new(router(state))
}

pub(crate) fn router(state: BraidAppState) -> Router {
    Router::new()
        .route("/{*resource}", get(handle_get).put(handle_put))
        .with_state(state)
}

async fn handle_get(
    State(state): State<BraidAppState>,
    Path(resource): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    RemoteId(peer): RemoteId,
    headers: HeaderMap,
) -> Response {
    if !state.can_read(&peer) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let mut policy_changes = state.policy_changed.subscribe();
    let url = SubscriptionManager::normalize_url(&resource);
    if braid_http::server::wants_subscribe(|name| headers.get(name).and_then(|v| v.to_str().ok())) {
        let parsed = match BraidHeaders::from_header_map(&headers) {
            Ok(parsed) if parsed.version.is_none() => parsed,
            _ => return StatusCode::BAD_REQUEST.into_response(),
        };
        let (history, mut receiver) = match state
            .resources
            .subscribe(&url, &parsed.parents.unwrap_or_default())
            .await
        {
            Ok(subscription) => subscription,
            Err(braid_http::BraidError::HistoryDropped) => return StatusCode::GONE.into_response(),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
        let stream = async_stream::try_stream! {
            for update in history { yield braid_http::protocol::format_update(&update)?; }
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(20));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                let event = tokio::select! {
                    biased;
                    changed = policy_changes.changed() => {
                        // A revoked peer's open stream ends at the next policy change.
                        if changed.is_err() || !state.can_read(&peer) { break; }
                        continue;
                    }
                    event = receiver.recv() => Some(event),
                    _ = interval.tick() => None,
                };
                if !state.can_read(&peer) { break; }
                match event {
                    Some(Ok(update)) => yield braid_http::protocol::format_update(&update)?,
                    Some(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                    Some(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => Err(braid_http::BraidError::HistoryDropped)?,
                    None => yield braid_http::protocol::format_heartbeat(),
                }
            }
        };
        let body = Body::from_stream(futures::StreamExt::map(
            stream,
            |item: braid_http::Result<Bytes>| item,
        ));
        return Response::builder()
            .status(209)
            .header("Subscribe", "true")
            .header("Heartbeats", "20")
            .header("Cache-Control", "no-cache")
            .header("Vary", "Subscribe, Version, Parents")
            .body(body)
            .unwrap_or_else(|error| {
                tracing::error!("Cannot construct subscription response: {error}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            });
    }
    let history = state.resources.history(&url).await;
    if let Some(version) = params.get("version") {
        return match history
            .into_iter()
            .find(|u| u.version.iter().any(|v| v.to_string() == *version))
        {
            Some(update) => Json(update).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        };
    }
    if params.get("history").is_some_and(|v| v == "true") {
        return Json(
            history
                .iter()
                .map(|u| {
                    u.version
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect::<Vec<_>>(),
        )
        .into_response();
    }
    match history.into_iter().last() {
        Some(update) => Json(update).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_put(
    State(state): State<BraidAppState>,
    Path(resource): Path<String>,
    RemoteId(peer): RemoteId,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !state.can_write(&peer) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let parsed = match BraidHeaders::from_header_map(&headers) {
        Ok(parsed) => parsed,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if parsed.patches_count.is_some() || parsed.content_range.is_some() {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let mut update = Update::snapshot(Version::new(uuid::Uuid::new_v4().to_string()), body);
    if let Some(version) = parsed.version {
        if version.is_empty() {
            return StatusCode::BAD_REQUEST.into_response();
        }
        update.version = version;
    }
    update.parents = parsed.parents.unwrap_or_default();
    update.merge_type = parsed.merge_type;
    update.content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let url = SubscriptionManager::normalize_url(&resource);
    state.resources.append(&url, update.clone()).await;
    if let Err(e) = state.subscriptions.broadcast(&url, &update).await {
        tracing::warn!("Gossip broadcast failed for {url}: {e}");
    }
    StatusCode::OK.into_response()
}
