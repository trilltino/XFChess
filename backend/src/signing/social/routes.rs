use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use super::friends::{Contact, FriendRequest};
use super::presence::{Presence, PresenceStatus};
use crate::signing::node_auth::{attach_node_identity, RequireNode};
use crate::signing::AppState;
use axum::http::HeaderMap;

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

const MAX_INVITES_PER_NODE: usize = 50;
const INVITE_TTL: chrono::Duration = chrono::Duration::hours(24);
const INVITE_SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3600);

pub fn spawn_invite_store_sweep(store: InviteStore) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(INVITE_SWEEP_INTERVAL);
        loop {
            interval.tick().await;
            let cutoff = Utc::now() - INVITE_TTL;
            let mut map = store.write();
            map.retain(|_, invites| {
                invites.retain(|inv| inv.received_at > cutoff);
                !invites.is_empty()
            });
        }
    });
}

fn err(msg: impl Into<String>) -> (StatusCode, Json<ErrorBody>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorBody { error: msg.into() }),
    )
}

/// Presence reads are public. Everything keyed by a node id requires that node's `X-Node-Auth`
/// token, so one player cannot read or change another's requests, contacts, presence or invites.
pub fn social_routes(invite_store: InviteStore) -> Router<AppState> {
    let public = Router::new()
        .route("/presence", get(get_presence))
        .route("/presence/summary", get(get_presence_summary))
        .route("/presence/{node_id}", get(get_node_presence));
    let node_routes = Router::new()
        .route(
            "/friends/requests",
            post(send_friend_request).get(list_pending_requests),
        )
        .route("/friends/requests/{id}", put(respond_friend_request))
        .route("/friends", get(list_friends))
        .route("/friends/{contact_id}", delete(remove_friend))
        .route("/presence", put(update_presence))
        .route(
            "/friends/invite",
            post({
                let store = invite_store.clone();
                move |state, node, body| push_lobby_invite(state, node, body, store)
            }),
        )
        .route(
            "/social/poll",
            get({
                let store = invite_store;
                move |state, node, query| poll_social(state, node, query, store)
            }),
        )
        .layer(axum::middleware::from_fn(attach_node_identity));
    public.merge(node_routes)
}

/// A wallet claimed in a social request counts only when the caller's bearer token is for that
/// wallet; otherwise it is dropped, so nobody can read or file requests as someone else's wallet.
async fn verified_wallet(
    state: &AppState,
    headers: &HeaderMap,
    claimed: Option<&str>,
) -> Option<String> {
    let claimed = claimed?;
    let wallet = crate::signing::routes::auth::authed_wallet(state, headers)
        .await
        .ok()?;
    (wallet == claimed).then_some(wallet)
}

fn forbidden(error: (StatusCode, String)) -> (StatusCode, Json<ErrorBody>) {
    (error.0, Json(ErrorBody { error: error.1 }))
}

async fn send_friend_request(
    State(state): State<AppState>,
    node: RequireNode,
    headers: HeaderMap,
    Json(body): Json<SendFriendRequestBody>,
) -> Result<Json<FriendRequest>, (StatusCode, Json<ErrorBody>)> {
    node.owns(&body.from_node_id).map_err(forbidden)?;
    let from_pubkey = verified_wallet(&state, &headers, body.from_pubkey.as_deref()).await;
    state
        .friends
        .send_request(
            body.from_node_id,
            from_pubkey,
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
    node: RequireNode,
    headers: HeaderMap,
    Query(q): Query<FriendQuery>,
) -> Result<Json<Vec<FriendRequest>>, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
    let pubkey = verified_wallet(&state, &headers, q.pubkey.as_deref()).await;
    state
        .friends
        .get_pending_requests(&q.node_id, pubkey.as_deref())
        .await
        .map(Json)
        .map_err(|e| err(e.to_string()))
}

async fn respond_friend_request(
    Path(request_id): Path<String>,
    State(state): State<AppState>,
    node: RequireNode,
    headers: HeaderMap,
    Json(body): Json<RespondBody>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    node.owns(&body.responder_node_id).map_err(forbidden)?;
    let wallet = crate::signing::routes::auth::authed_wallet(&state, &headers)
        .await
        .ok();
    // Only the addressee may answer: the request's node, or the wallet it was addressed to.
    if !state
        .friends
        .is_addressee(&request_id, &node.0, wallet.as_deref())
        .await
        .map_err(|e| err(e.to_string()))?
    {
        return Err(forbidden((
            StatusCode::FORBIDDEN,
            "This friend request is not addressed to you".to_string(),
        )));
    }
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
    node: RequireNode,
    Query(q): Query<FriendQuery>,
) -> Result<Json<Vec<Contact>>, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
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
    node: RequireNode,
    Query(q): Query<FriendQuery>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
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

#[derive(Serialize)]
struct PresenceSummary {
    online_count: usize,
    games_in_progress: usize,
}

async fn get_presence_summary(State(state): State<AppState>) -> Json<PresenceSummary> {
    Json(PresenceSummary {
        online_count: state.presence.count_in_game(),
        games_in_progress: state.presence.count_games_in_progress(),
    })
}

async fn get_node_presence(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Json<Option<Presence>> {
    Json(state.presence.get_online(&node_id))
}

async fn update_presence(
    State(state): State<AppState>,
    node: RequireNode,
    headers: HeaderMap,
    Json(mut p): Json<Presence>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    node.owns(&p.node_id).map_err(forbidden)?;
    p.pubkey = verified_wallet(&state, &headers, p.pubkey.as_deref()).await;
    // Freshness decides who counts as online, so the clock is the server's, not the caller's.
    p.updated_at = Utc::now();
    state.presence.upsert(p);
    Ok(Json(
        serde_json::json!({ "online_count": state.presence.count_online() }),
    ))
}

async fn push_lobby_invite(
    State(_state): State<AppState>,
    node: RequireNode,
    Json(body): Json<LobbyInviteBody>,
    store: InviteStore,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    node.owns(&body.from_node_id).map_err(forbidden)?;
    let invite = LobbyInvite {
        game_id: body.game_id,
        from_node_id: body.from_node_id,
        from_display: body.from_display,
        received_at: Utc::now(),
    };
    {
        let mut map = store.write();
        let invites = map.entry(body.to_node_id).or_default();
        invites.push(invite);
        // Oldest-first: if we're over the cap, drop from the front.
        if invites.len() > MAX_INVITES_PER_NODE {
            let overflow = invites.len() - MAX_INVITES_PER_NODE;
            invites.drain(0..overflow);
        }
    }
    Ok(StatusCode::OK)
}

async fn poll_social(
    State(_state): State<AppState>,
    node: RequireNode,
    Query(q): Query<SocialPollQuery>,
    store: InviteStore,
) -> Result<Json<SocialPollResponse>, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
    let map = store.read();
    let all = map.get(&q.node_id).map(|v| v.as_slice()).unwrap_or(&[]);
    let since = q.since_index.unwrap_or(0);
    let invites = all.get(since..).unwrap_or(&[]).to_vec();
    let next_index = since + invites.len();
    Ok(Json(SocialPollResponse {
        invites,
        next_index,
    }))
}
