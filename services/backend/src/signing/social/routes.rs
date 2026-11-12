use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::friends::{BlockedPlayer, Contact, FriendRequest};
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
    /// Add a friend by account username instead of by ID; resolved to their wallet here.
    #[serde(default)]
    pub to_username: Option<String>,
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct RespondBody {
    pub action: String, // "accept" | "reject"
    pub responder_node_id: String,
    pub responder_display: Option<String>,
}

#[derive(Deserialize)]
pub struct FriendQuery {
    pub node_id: String,
    pub pubkey: Option<String>,
    pub outgoing: Option<bool>,
}

#[derive(Deserialize)]
pub struct BlockBody {
    pub blocker_node_id: String,
    pub blocked_node_id: String,
}

#[derive(Deserialize)]
pub struct LobbyInviteBody {
    pub game_id: String,
    pub from_node_id: String,
    pub from_display: String,
    pub to_node_id: String,
    #[serde(default)]
    pub direct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyInvite {
    pub id: String,
    pub game_id: String,
    pub from_node_id: String,
    pub from_display: String,
    #[serde(default)]
    pub direct_node_id: Option<String>,
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

fn err(msg: impl Into<String>) -> (StatusCode, Json<ErrorBody>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorBody { error: msg.into() }),
    )
}

/// Presence reads are public. Everything keyed by a node id requires that node's `X-Node-Auth`
/// token, so one player cannot read or change another's requests, contacts, presence or invites.
pub fn social_routes() -> Router<AppState> {
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
        .route("/friends/requests/{id}", delete(cancel_friend_request))
        .route("/friends/invites/{id}", delete(dismiss_invite))
        .route("/friends", get(list_friends))
        .route("/friends/blocks", get(list_blocks).post(block_player))
        .route("/friends/blocks/{blocked_node_id}", delete(unblock_player))
        .route("/friends/{contact_id}", delete(remove_friend))
        .route("/presence", put(update_presence))
        .route("/friends/invite", post(push_lobby_invite))
        .route("/social/poll", get(poll_social))
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

/// Username lookups reveal whether an account exists, so they are rationed: a few per node per
/// minute, and a ceiling across all nodes because node keys cost nothing to mint.
const LOOKUPS_PER_NODE_PER_MINUTE: usize = 10;
const LOOKUPS_PER_MINUTE: usize = 300;
const LOOKUP_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);
const LOOKUP_NODE_TABLE_LIMIT: usize = 4096;

#[derive(Default)]
struct LookupLimiter {
    per_node: std::collections::HashMap<String, Vec<std::time::Instant>>,
    all: Vec<std::time::Instant>,
}

impl LookupLimiter {
    fn allow(&mut self, node: &str, now: std::time::Instant) -> bool {
        self.all.retain(|at| now.duration_since(*at) < LOOKUP_WINDOW);
        if self.per_node.len() > LOOKUP_NODE_TABLE_LIMIT {
            self.per_node.retain(|_, hits| {
                hits.retain(|at| now.duration_since(*at) < LOOKUP_WINDOW);
                !hits.is_empty()
            });
        }
        let hits = self.per_node.entry(node.to_string()).or_default();
        hits.retain(|at| now.duration_since(*at) < LOOKUP_WINDOW);
        if hits.len() >= LOOKUPS_PER_NODE_PER_MINUTE || self.all.len() >= LOOKUPS_PER_MINUTE {
            return false;
        }
        hits.push(now);
        self.all.push(now);
        true
    }
}

fn allow_lookup(node: &str) -> bool {
    static LIMITER: std::sync::OnceLock<parking_lot::Mutex<LookupLimiter>> =
        std::sync::OnceLock::new();
    LIMITER
        .get_or_init(Default::default)
        .lock()
        .allow(node, std::time::Instant::now())
}

async fn send_friend_request(
    State(state): State<AppState>,
    node: RequireNode,
    headers: HeaderMap,
    Json(mut body): Json<SendFriendRequestBody>,
) -> Result<Json<FriendRequest>, (StatusCode, Json<ErrorBody>)> {
    node.owns(&body.from_node_id).map_err(forbidden)?;
    if let Some(username) = body.to_username.take() {
        let username = username.trim();
        if body.to_node_id.is_some() || body.to_pubkey.is_some() {
            return Err(err("Name the player by username or by ID, not both"));
        }
        if username.is_empty()
            || username.chars().count() > 30
            || username.chars().any(char::is_control)
        {
            return Err(err("Invalid username"));
        }
        if !allow_lookup(&node.0) {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorBody {
                    error: "Too many username lookups. Try again in a minute.".into(),
                }),
            ));
        }
        let (wallet, _) = state
            .friends
            .resolve_username(username)
            .await
            .map_err(|e| err(e.to_string()))?
            .ok_or_else(|| err("No player has that username"))?;
        body.to_pubkey = Some(wallet);
    }
    if body.from_display.trim().is_empty()
        || body.from_display.chars().count() > 64
        || body
            .message
            .as_ref()
            .is_some_and(|message| message.chars().count() > 500)
    {
        return Err(err("Invalid display name or request message"));
    }
    for key in [body.to_node_id.as_deref(), body.to_pubkey.as_deref()]
        .into_iter()
        .flatten()
    {
        if key.parse::<solana_sdk::pubkey::Pubkey>().is_err() {
            return Err(err("Invalid friend ID"));
        }
    }
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
    if q.outgoing.unwrap_or(false) {
        return state
            .friends
            .outgoing_requests(&q.node_id)
            .await
            .map(Json)
            .map_err(|e| err(e.to_string()));
    }
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
    let accept = match body.action.as_str() {
        "accept" => true,
        "reject" => false,
        _ => return Err(err("Unknown friend request response")),
    };
    let display = body
        .responder_display
        .as_deref()
        .filter(|name| !name.trim().is_empty() && name.chars().count() <= 64)
        .unwrap_or(&body.responder_node_id);
    state
        .friends
        .respond_to_request(
            &request_id,
            accept,
            &body.responder_node_id,
            display,
            wallet.as_deref(),
        )
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| err(e.to_string()))
}

async fn list_friends(
    State(state): State<AppState>,
    node: RequireNode,
    headers: HeaderMap,
    Query(q): Query<FriendQuery>,
) -> Result<Json<Vec<Contact>>, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
    // A signed-in wallet carries its friendships to whichever device it plays from.
    if let Ok(wallet) = crate::signing::routes::auth::authed_wallet(&state, &headers).await {
        if let Err(error) = state.friends.sync_wallet_contacts(&q.node_id, &wallet).await {
            tracing::warn!("[Friends] wallet contact sync failed: {error}");
        }
    }
    let mut contacts = state
        .friends
        .get_contacts(&q.node_id)
        .await
        .map_err(|e| err(e.to_string()))?;

    // Annotate online status from presence store
    for c in &mut contacts {
        if let Some(p) = state.presence.get_online(&c.contact_node_id) {
            c.is_online = p.status == PresenceStatus::Online || p.status == PresenceStatus::InGame;
            c.contact_display = p.display_name;
            c.contact_pubkey = p.pubkey.or(c.contact_pubkey.clone());
            c.last_seen = Some(p.updated_at);
        }
    }
    Ok(Json(contacts))
}

async fn remove_friend(
    Path(contact_id): Path<String>,
    State(state): State<AppState>,
    node: RequireNode,
    headers: HeaderMap,
    Query(q): Query<FriendQuery>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
    let wallet = crate::signing::routes::auth::authed_wallet(&state, &headers)
        .await
        .ok();
    state
        .friends
        .remove_contact(&q.node_id, &contact_id, wallet.as_deref())
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| err(e.to_string()))
}

#[derive(Deserialize)]
struct PresenceQuery {
    wallet: Option<String>,
}

async fn get_presence(
    State(state): State<AppState>,
    Query(query): Query<PresenceQuery>,
) -> Json<Vec<Presence>> {
    Json(
        state
            .presence
            .get_all_online()
            .into_iter()
            .filter(|p| {
                query
                    .wallet
                    .as_ref()
                    .is_none_or(|wallet| p.pubkey.as_ref() == Some(wallet))
            })
            .collect(),
    )
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
    p.transport_node_id = match headers.get("x-transport-auth") {
        Some(token) => Some(
            crate::signing::node_auth::verify_token(
                token.to_str().map_err(|_| err("Invalid transport proof"))?,
                Utc::now().timestamp(),
            )
            .map_err(|_| err("Invalid transport proof"))?,
        ),
        None => None,
    };
    p.pubkey = verified_wallet(&state, &headers, p.pubkey.as_deref()).await;
    // Freshness decides who counts as online, so the clock is the server's, not the caller's.
    p.updated_at = Utc::now();
    // Heartbeats are frequent; write the wallet's node only when it changes or after a gap.
    if let Some(wallet) = p.pubkey.as_deref() {
        let unchanged = state.presence.get(&p.node_id).is_some_and(|previous| {
            previous.pubkey.as_deref() == Some(wallet)
                && p.updated_at.signed_duration_since(previous.updated_at)
                    < chrono::Duration::seconds(30)
        });
        if !unchanged {
            if let Err(error) = state
                .friends
                .note_wallet_node(wallet, &p.node_id, &p.display_name)
                .await
            {
                tracing::warn!("[Friends] wallet node update failed: {error}");
            }
        }
    }
    state.presence.upsert(p);
    Ok(Json(
        serde_json::json!({ "online_count": state.presence.count_online() }),
    ))
}

async fn push_lobby_invite(
    State(state): State<AppState>,
    node: RequireNode,
    headers: HeaderMap,
    Json(body): Json<LobbyInviteBody>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    node.owns(&body.from_node_id).map_err(forbidden)?;
    let transport = match headers.get("x-transport-auth") {
        Some(token) => crate::signing::node_auth::verify_token(
            token.to_str().map_err(|_| err("Invalid transport proof"))?,
            Utc::now().timestamp(),
        )
        .map_err(|_| forbidden((StatusCode::UNAUTHORIZED, "Invalid transport proof".into())))?,
        None if !body.direct => node.0.clone(), // legacy public-lobby client
        None => return Err(err("Direct invitations require a transport proof")),
    };
    let sender_wallet = crate::signing::routes::auth::authed_wallet(&state, &headers)
        .await
        .ok();
    let recipient_wallet = state
        .friends
        .wallet_of_node(&body.to_node_id)
        .await
        .map_err(|e| err(e.to_string()))?;
    if state
        .friends
        .blocked_between(
            (&node.0, sender_wallet.as_deref()),
            (&body.to_node_id, recipient_wallet.as_deref()),
        )
        .await
        .map_err(|e| err(e.to_string()))?
    {
        return Err(forbidden((
            StatusCode::FORBIDDEN,
            "This player cannot receive your invitation".into(),
        )));
    }
    let profile_friends = state
        .friends
        .are_friends(&node.0, &body.to_node_id)
        .await
        .map_err(|e| err(e.to_string()))?;
    let wallet_friends = match (sender_wallet.as_deref(), recipient_wallet.as_deref()) {
        (Some(mine), Some(theirs)) => state
            .friends
            .wallets_are_friends(mine, theirs)
            .await
            .map_err(|e| err(e.to_string()))?,
        _ => false,
    };
    if !profile_friends && !wallet_friends {
        return Err(forbidden((
            StatusCode::FORBIDDEN,
            "Accept a friend request before sending invitations".into(),
        )));
    }
    if body.game_id.len() > 80 || body.from_display.chars().count() > 64 {
        return Err(err("Invalid invitation"));
    }
    if !body.direct {
        let rooms = state.p2p_relay.read();
        let room = rooms
            .get(&body.game_id)
            .ok_or_else(|| err("This lobby has closed"))?;
        if room.announcement.host_node_id != transport
            || room.announcement.status != crate::signing::p2p_relay::types::GameStatus::Open
            || room.announcement.stake_amount != 0.0
            || room.announcement.password_hash.is_some()
        {
            return Err(err(
                "Only the host of an open, free lobby can invite friends",
            ));
        }
    }
    let invite = LobbyInvite {
        id: uuid::Uuid::new_v4().to_string(),
        game_id: body.game_id,
        from_node_id: body.from_node_id,
        from_display: body.from_display,
        direct_node_id: body.direct.then_some(transport),
        received_at: Utc::now(),
    };
    state
        .friends
        .save_invite(&body.to_node_id, &invite)
        .await
        .map_err(|e| err(e.to_string()))?;
    Ok(StatusCode::OK)
}

async fn poll_social(
    State(state): State<AppState>,
    node: RequireNode,
    Query(q): Query<SocialPollQuery>,
) -> Result<Json<SocialPollResponse>, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
    let mut invites = state
        .friends
        .invites(&q.node_id)
        .await
        .map_err(|e| err(e.to_string()))?;
    invites.retain(|invite| {
        if invite.direct_node_id.is_some() {
            state
                .presence
                .get_online(&invite.from_node_id)
                .is_some_and(|presence| {
                    presence.status == PresenceStatus::Online
                        && presence.game_id.as_deref() == Some(&invite.game_id)
                })
        } else {
            state
                .p2p_relay
                .read()
                .get(&invite.game_id)
                .is_some_and(|room| {
                    room.announcement.status == crate::signing::p2p_relay::types::GameStatus::Open
                        && Utc::now().signed_duration_since(room.last_activity)
                            < chrono::Duration::seconds(
                                crate::signing::p2p_relay::types::LOBBY_TTL_SECS,
                            )
                })
        }
    });
    let next_index = invites.len();
    Ok(Json(SocialPollResponse {
        invites,
        next_index,
    }))
}

async fn cancel_friend_request(
    State(state): State<AppState>,
    node: RequireNode,
    Path(id): Path<String>,
    Query(q): Query<FriendQuery>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
    let removed = state
        .friends
        .cancel_request(&id, &node.0)
        .await
        .map_err(|e| err(e.to_string()))?;
    if !removed {
        return Err(forbidden((
            StatusCode::FORBIDDEN,
            "This pending request does not belong to you".into(),
        )));
    }
    Ok(StatusCode::OK)
}

async fn dismiss_invite(
    State(state): State<AppState>,
    node: RequireNode,
    Path(id): Path<String>,
    Query(q): Query<FriendQuery>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
    state
        .friends
        .dismiss_invite(&node.0, &id)
        .await
        .map_err(|e| err(e.to_string()))?;
    Ok(StatusCode::OK)
}

async fn list_blocks(
    State(state): State<AppState>,
    node: RequireNode,
    Query(q): Query<FriendQuery>,
) -> Result<Json<Vec<BlockedPlayer>>, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
    state
        .friends
        .blocked_players(&q.node_id)
        .await
        .map(Json)
        .map_err(|e| err(e.to_string()))
}

async fn block_player(
    State(state): State<AppState>,
    node: RequireNode,
    headers: HeaderMap,
    Json(body): Json<BlockBody>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    node.owns(&body.blocker_node_id).map_err(forbidden)?;
    if body
        .blocked_node_id
        .parse::<solana_sdk::pubkey::Pubkey>()
        .is_err()
    {
        return Err(err("Invalid player ID"));
    }
    let wallet = crate::signing::routes::auth::authed_wallet(&state, &headers)
        .await
        .ok();
    state
        .friends
        .block(&body.blocker_node_id, wallet.as_deref(), &body.blocked_node_id)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| err(e.to_string()))
}

async fn unblock_player(
    State(state): State<AppState>,
    node: RequireNode,
    Path(blocked_node_id): Path<String>,
    Query(q): Query<FriendQuery>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    node.owns(&q.node_id).map_err(forbidden)?;
    state
        .friends
        .unblock(&q.node_id, &blocked_node_id)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| err(e.to_string()))
}
