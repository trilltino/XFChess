//! HTTP handlers for the social subsystem:
//!   GET/POST /friends
//!   GET/PUT  /friends/requests
//!   GET/PUT  /presence
//!   POST     /friends/invite   (lobby invite push)
//!   GET      /social/poll      (pull pending invites by node_id)

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::friends::{Contact, FriendRequest};
use super::presence::{Presence, PresenceStatus};
use crate::signing::AppState;

// ── Request / response DTOs ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SendFriendRequestBody {
    pub from_node_id: String,
    pub from_pubkey: Option<String>,
    pub from_display: String,
    pub to_node_id: Option<String>,
    pub to_pubkey: Option<String>,
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct RespondBody {
    pub action: String, // "accept" | "reject"
    pub responder_node_id: String,
}

#[derive(Deserialize)]
pub struct FriendQuery {
    pub node_id: String,
    pub pubkey: Option<String>,
}

#[derive(Deserialize)]
pub struct LobbyInviteBody {
    pub game_id: String,
    pub from_node_id: String,
    pub from_display: String,
    pub to_node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyInvite {
    pub game_id: String,
    pub from_node_id: String,
    pub from_display: String,
    pub received_at: chrono::DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct SocialPollQuery {
    pub node_id: String,
    pub since_index: Option<usize>,
}

#[derive(Serialize)]
pub struct SocialPollResponse {
    pub invites: Vec<LobbyInvite>,
    pub next_index: usize,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

type InviteStore = Arc<RwLock<HashMap<String, Vec<LobbyInvite>>>>;

fn err(msg: impl Into<String>) -> (StatusCode, Json<ErrorBody>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorBody { error: msg.into() }),
    )
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn social_routes(invite_store: InviteStore) -> Router<AppState> {
    Router::new()
        .route(
            "/friends/requests",
            post(send_friend_request).get(list_pending_requests),
        )
        .route("/friends/requests/{id}", put(respond_friend_request))
        .route("/friends", get(list_friends))
        .route("/friends/{contact_id}", delete(remove_friend))
        .route("/presence", get(get_presence).put(update_presence))
        .route(
            "/friends/invite",
            post({
                let store = invite_store.clone();
                move |state, body| push_lobby_invite(state, body, store)
            }),
        )
        .route(
            "/social/poll",
            get({
                let store = invite_store.clone();
                move |state, query| poll_social(state, query, store)
            }),
        )
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn send_friend_request(
    State(state): State<AppState>,
    Json(body): Json<SendFriendRequestBody>,
) -> Result<Json<FriendRequest>, (StatusCode, Json<ErrorBody>)> {
    state
        .friends
        .send_request(
            body.from_node_id,
            body.from_pubkey,
            body.from_display,
            body.to_node_id,
            body.to_pubkey,
            body.message,
        )
        .await
        .map(Json)
        .map_err(|e| err(e.to_string()))
}

async fn list_pending_requests(
    State(state): State<AppState>,
    Query(q): Query<FriendQuery>,
) -> Result<Json<Vec<FriendRequest>>, (StatusCode, Json<ErrorBody>)> {
    state
        .friends
        .get_pending_requests(&q.node_id, q.pubkey.as_deref())
        .await
        .map(Json)
        .map_err(|e| err(e.to_string()))
}

async fn respond_friend_request(
    Path(request_id): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<RespondBody>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    let accept = body.action == "accept";
    state
        .friends
        .respond_to_request(&request_id, accept, &body.responder_node_id)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| err(e.to_string()))
}

async fn list_friends(
    State(state): State<AppState>,
    Query(q): Query<FriendQuery>,
) -> Result<Json<Vec<Contact>>, (StatusCode, Json<ErrorBody>)> {
    let mut contacts = state
        .friends
        .get_contacts(&q.node_id)
        .await
        .map_err(|e| err(e.to_string()))?;

    // Annotate online status from presence store
    for c in &mut contacts {
        if let Some(p) = state.presence.get(&c.contact_node_id) {
            c.is_online = p.status == PresenceStatus::Online || p.status == PresenceStatus::InGame;
            c.last_seen = Some(p.updated_at);
        }
    }
    Ok(Json(contacts))
}

async fn remove_friend(
    Path(contact_id): Path<String>,
    State(state): State<AppState>,
    Query(q): Query<FriendQuery>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    state
        .friends
        .remove_contact(&q.node_id, &contact_id)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| err(e.to_string()))
}

async fn get_presence(State(state): State<AppState>) -> Json<Vec<Presence>> {
    Json(state.presence.get_all_online())
}

async fn update_presence(State(state): State<AppState>, Json(p): Json<Presence>) -> StatusCode {
    state.presence.upsert(p);
    StatusCode::OK
}

async fn push_lobby_invite(
    State(_state): State<AppState>,
    Json(body): Json<LobbyInviteBody>,
    store: InviteStore,
) -> StatusCode {
    let invite = LobbyInvite {
        game_id: body.game_id,
        from_node_id: body.from_node_id,
        from_display: body.from_display,
        received_at: Utc::now(),
    };
    store
        .write()
        .unwrap()
        .entry(body.to_node_id)
        .or_default()
        .push(invite);
    StatusCode::OK
}

async fn poll_social(
    State(_state): State<AppState>,
    Query(q): Query<SocialPollQuery>,
    store: InviteStore,
) -> Json<SocialPollResponse> {
    let map = store.read().unwrap();
    let all = map.get(&q.node_id).map(|v| v.as_slice()).unwrap_or(&[]);
    let since = q.since_index.unwrap_or(0);
    let invites = all.get(since..).unwrap_or(&[]).to_vec();
    let next_index = since + invites.len();
    Json(SocialPollResponse {
        invites,
        next_index,
    })
}
