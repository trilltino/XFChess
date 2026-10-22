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
use iroh_h3_axum::IrohAxum;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub(crate) struct BraidAppState {
    pub subscriptions: Arc<SubscriptionManager>,
    pub resources: Arc<ResourceStore>,
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
    headers: HeaderMap,
) -> Response {
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
                    event = receiver.recv() => Some(event),
                    _ = interval.tick() => None,
                };
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
    headers: HeaderMap,
    body: Bytes,
) -> Response {
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
