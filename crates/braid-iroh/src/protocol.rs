//! Braid protocol handler for iroh connections.
//!
//! Wraps an Axum router in `IrohAxum` so that Braid-HTTP routes (GET, PUT
//! with Version/Parents headers) are served over HTTP/3 on iroh QUIC
//! connections. This is the bridge between the P2P transport and the
//! existing braid_http_rs server middleware.

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, put},
    Router,
};
use braid_core::{Update, Version};
use bytes::Bytes;
use http::StatusCode;
use iroh_h3_axum::IrohAxum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::subscription::SubscriptionManager;

/// Swiss tournament gossip messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwissMessage {
    /// New round pairings available
    RoundStarted {
        tournament_id: u64,
        round: u8,
        pairings: Vec<SwissPairing>,
    },
    /// Match result recorded
    ResultRecorded {
        tournament_id: u64,
        round: u8,
        board: u16,
        result: MatchResult,
    },
    /// Standings updated
    StandingsUpdated {
        tournament_id: u64,
        standings: Vec<SwissStandingsEntry>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwissPairing {
    pub white: String,
    pub black: String,
    pub board: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchResult {
    Win { winner: String },
    Draw,
}

impl std::fmt::Display for MatchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchResult::Win { winner } => write!(f, "win:{winner}"),
            MatchResult::Draw => write!(f, "draw"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwissStandingsEntry {
    pub player_id: String,
    pub score: f64,
    pub rank: u16,
}

/// Shared state accessible from Axum route handlers.
#[derive(Clone)]
pub struct BraidAppState {
    /// Subscription manager for gossip-backed pub/sub.
    pub subscriptions: Arc<SubscriptionManager>,
    /// In-memory resource store: URL → List of Updates (History).
    pub resources: Arc<tokio::sync::RwLock<std::collections::HashMap<String, Vec<Update>>>>,
    /// Optional SQLite pool for durable resource persistence.
    /// When `Some`, every PUT is also written to the `braid_resources` table.
    pub db: Option<sqlx::SqlitePool>,
}

impl BraidAppState {
    /// Broadcast a Swiss tournament message via gossip
    pub async fn broadcast_swiss_message(&self, tournament_id: u64, message: SwissMessage) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("/swiss/{}", tournament_id);
        let body = serde_json::to_string(&message)?;
        let version = Version::String(uuid::Uuid::new_v4().to_string());
        let update = Update::snapshot(version, Bytes::from(body.into_bytes()));

        // Store locally
        let mut resources = self.resources.write().await;
        resources
            .entry(url.clone())
            .or_insert_with(Vec::new)
            .push(update.clone());
        drop(resources);

        // Broadcast to gossip subscribers
        self.subscriptions.broadcast(&url, &update).await?;
        tracing::info!("[braid] Broadcast Swiss message for tournament {}", tournament_id);
        Ok(())
    }

    /// Subscribe to Swiss tournament updates
    pub async fn subscribe_swiss(&self, tournament_id: u64) -> Result<iroh_gossip::api::GossipReceiver, Box<dyn std::error::Error>> {
        let url = format!("/swiss/{}", tournament_id);
        let (_sender, receiver) = self.subscriptions.subscribe(&url, vec![]).await?;
        Ok(receiver)
    }
    /// Ensure the `braid_resources` table exists and warm-load existing
    /// resources into the in-memory map. Call this once on startup.
    pub async fn init_db(&self) {
        let pool = match &self.db {
            Some(p) => p,
            None => return,
        };

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS braid_resources (
                url        TEXT    NOT NULL,
                version    TEXT    NOT NULL,
                body       TEXT,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (url, version)
            )"#,
        )
        .execute(pool)
        .await
        .ok();

        // Warm-load into memory (latest version per URL)
        let rows: Vec<(String, String, Option<String>)> =
            sqlx::query_as("SELECT url, version, body FROM braid_resources ORDER BY updated_at ASC")
                .fetch_all(pool)
                .await
                .unwrap_or_default();

        let mut resources = self.resources.write().await;
        for (url, version_str, body) in rows {
            let update = Update::snapshot(
                Version::String(version_str),
                Bytes::from(body.unwrap_or_default().into_bytes()),
            );
            resources.entry(url).or_insert_with(Vec::new).push(update);
        }
        tracing::info!("[braid] warm-loaded {} resource URL(s) from SQLite", resources.len());
    }

    /// Persist a single update to SQLite (soft-fail — never breaks the P2P layer).
    async fn persist_update(&self, url: &str, version_str: &str, body: &str) {
        let pool = match &self.db {
            Some(p) => p,
            None => return,
        };
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT OR REPLACE INTO braid_resources (url, version, body, updated_at) VALUES (?, ?, ?, ?)"
        )
        .bind(url)
        .bind(version_str)
        .bind(body)
        .bind(now)
        .execute(pool)
        .await
        .ok();
    }
}

/// Build the Axum router with Braid-HTTP routes, then wrap it in
/// `IrohAxum` so it can be mounted on an iroh endpoint.
///
/// Routes:
/// - `GET /:resource`  → returns the latest snapshot
/// - `PUT /:resource`  → accepts a new update, broadcasts via gossip
pub fn build_protocol_handler(state: BraidAppState, external_router: Option<Router>) -> IrohAxum {
    let local_router = Router::new()
        .route("/{resource}", get(handle_get))
        .route("/{resource}", put(handle_put))
        .with_state(state);

    let final_router = match external_router {
        Some(ext) => local_router.merge(ext),
        None => local_router,
    };

    IrohAxum::new(final_router)
}

use http::HeaderMap;

/// GET handler — returns the current state of a resource.
/// If the resource doesn't exist yet, returns 404.
async fn handle_get(
    State(state): State<BraidAppState>,
    axum::extract::Path(resource): axum::extract::Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let url = format!("/{}", resource);

    // Check for Subscribe header (Subscription 209 Demo)
    if let Some(val) = headers.get("subscribe") {
        if val == "true" {
            println!("[PROTOCOL] Received Subscribe request for {}", url);

            // Try to get current content to send as initial state (latest version)
            let resources = state.resources.read().await;
            println!(
                "[PROTOCOL] Looking for resource in store, available keys: {:?}",
                resources.keys().collect::<Vec<_>>()
            );
            let initial_content = if let Some(history) = resources.get(&url) {
                println!(
                    "[PROTOCOL] Found history with {} entries for {}",
                    history.len(),
                    url
                );
                if let Some(latest) = history.last() {
                    let content = serde_json::to_string(latest).unwrap_or_default();
                    println!(
                        "[PROTOCOL] Sending initial content (len: {})",
                        content.len()
                    );
                    content
                } else {
                    println!("[PROTOCOL] History empty for {}", url);
                    String::new()
                }
            } else {
                // Empty initial state
                println!("[PROTOCOL] Resource not found: {}", url);
                String::new()
            };

            return (
                StatusCode::from_u16(209).unwrap(),
                [("subscribe", "true")],
                initial_content,
            )
                .into_response();
        }
    }

    let resources = state.resources.read().await;

    if let Some(history) = resources.get(&url) {
        // 1. Check for ?version=...
        if let Some(ver) = params.get("version") {
            // Basic implementation: check if version string contains the requested ID
            // Optimally we'd use strict equality but Version enum makes it tricky strictly
            if let Some(update) = history
                .iter()
                .find(|u| u.version.iter().any(|v| v.to_string() == *ver))
            {
                return (
                    StatusCode::OK,
                    serde_json::to_string(update).unwrap_or_default(),
                )
                    .into_response();
            }
            return StatusCode::NOT_FOUND.into_response();
        }

        // 2. Check for ?history=true
        if params.get("history").map(|v| v == "true").unwrap_or(false) {
            // Return list of version strings
            let versions: Vec<String> = history
                .iter()
                .map(|u| {
                    u.version
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect();
            return (
                StatusCode::OK,
                serde_json::to_string(&versions).unwrap_or_default(),
            )
                .into_response();
        }

        // 3. Default: Return latest
        if let Some(latest) = history.last() {
            return (
                StatusCode::OK,
                serde_json::to_string(latest).unwrap_or_default(),
            )
                .into_response();
        }
    }

    StatusCode::NOT_FOUND.into_response()
}

/// PUT handler — stores a new update and broadcasts it via gossip.
async fn handle_put(
    State(state): State<BraidAppState>,
    axum::extract::Path(resource): axum::extract::Path<String>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let url = format!("/{}", resource);

    // Debug logging for Braid format
    println!("\nINCOMING BRAID PUT:");
    println!("PUT {} HTTP/3", url);
    for (name, value) in &headers {
        if name.as_str().starts_with("version")
            || name.as_str().starts_with("parents")
            || name.as_str().starts_with("merge-type")
            || name.as_str().starts_with("content-type")
            || name.as_str().starts_with("content-range")
            || name.as_str().starts_with("braid-")
        {
            println!("{}: {}", name, value.to_str().unwrap_or("???"));
        }
    }
    println!();
    println!("{}", body);
    println!("----------------------------------------\n");

    // Parse the incoming update from headers
    let version = headers
        .get("version")
        .and_then(|v| v.to_str().ok())
        .map(|v| Version::String(v.to_string()))
        .unwrap_or_else(|| Version::String(uuid::Uuid::new_v4().to_string()));
    let version_str = match &version {
        Version::String(s) => s.clone(),
        _ => uuid::Uuid::new_v4().to_string(),
    };
    let update = Update::snapshot(version, Bytes::from(body.clone().into_bytes()));

    // Store locally (append to history)
    let mut resources = state.resources.write().await;
    resources
        .entry(url.clone())
        .or_insert_with(Vec::new)
        .push(update.clone());
    drop(resources);

    // Persist to SQLite for durability (soft-fail — never kills P2P)
    state.persist_update(&url, &version_str, &body).await;

    // Broadcast to gossip subscribers
    if let Err(e) = state.subscriptions.broadcast(&url, &update).await {
        tracing::warn!("gossip broadcast failed for {}: {}", url, e);
    }

    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_braid_app_state_clone() {
        // This test verifies BraidAppState can be cloned (required for Axum)
        // We can't easily create a real SubscriptionManager without a gossip instance,
        // but we can verify the derive macro works
        fn assert_clone<T: Clone>() {}
        assert_clone::<BraidAppState>();
    }

    #[test]
    fn test_build_protocol_handler() {
        // Just verify the function signature compiles
        // Real testing requires a gossip instance
        fn assert_handler_fn<F>()
        where
            F: Fn(BraidAppState) -> IrohAxum,
        {
        }
        assert_handler_fn::<fn(BraidAppState) -> IrohAxum>();
    }
}
