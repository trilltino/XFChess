//! HTTP route handlers for P2P relay.

use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::Json as AxumJson,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::Deserialize;

use crate::signing::AppState;

use super::types::{
    AcceptJoinReq, ActiveGame, AnnounceGameRequest, AnnounceGameResponse, GameListing, GameStatus,
    HeartbeatRequest, JoinGameRequest, JoinGameResponse, LeaveGameRequest, PollMessagesRequest,
    PollMessagesResponse, SendMessageRequest,
};

/// Optional filter / sort parameters for GET /p2p/games
#[derive(Debug, Default, Deserialize)]
pub struct LobbyFilter {
    pub time_min: Option<u32>,
    pub time_max: Option<u32>,
    pub stake_min: Option<f64>,
    pub stake_max: Option<f64>,
    pub elo_min: Option<u16>,
    pub elo_max: Option<u16>,
    /// "elo_asc" | "elo_desc" | "stake_asc" | "stake_desc" | "time_asc" | "newest"
    pub sort: Option<String>,
}

/// Creates the P2P relay router
pub fn p2p_routes() -> Router<AppState> {
    Router::new()
        .route("/p2p/announce", post(announce_game))
        .route("/p2p/games", get(list_games))
        .route("/p2p/join", post(join_game))
        .route("/p2p/accept", post(accept_join))
        .route("/p2p/leave", post(leave_game))
        .route("/p2p/heartbeat", post(heartbeat_game))
        .route("/p2p/message", post(send_message))
        .route("/p2p/poll", post(poll_messages))
        .route("/region", get(get_region))
}

/// Announce a new P2P game
pub async fn announce_game(
    State(state): State<AppState>,
    Json(req): Json<AnnounceGameRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let game_id_clone = req.game_id.clone();
    let host_node_id_clone = req.host_node_id.clone();

    let password_hash = req
        .password
        .as_deref()
        .map(|p| bcrypt::hash(p, 10).unwrap_or_default());

    let announcement = super::types::P2PGameAnnouncement {
        game_id: req.game_id,
        host_node_id: req.host_node_id,
        display_name: req.display_name,
        stake_amount: req.stake_amount,
        game_type: req.game_type,
        base_time_seconds: req.base_time_seconds,
        increment_seconds: req.increment_seconds,
        created_at: Utc::now(),
        status: GameStatus::Open,
        username: req.username,
        elo: req.elo,
        region: req.region,
        password_hash,
    };

    let active_game = ActiveGame {
        announcement,
        joiner_node_id: None,
        host_messages: Vec::new(),
        joiner_messages: Vec::new(),
        last_activity: Utc::now(),
        pending_invites: Vec::new(),
    };

    games.insert(game_id_clone.clone(), active_game);

    tracing::info!(
        "P2P game announced: {} by {}",
        game_id_clone,
        host_node_id_clone
    );

    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

/// List all open P2P games (supports optional filter/sort query params)
pub async fn list_games(
    State(state): State<AppState>,
    Query(filter): Query<LobbyFilter>,
) -> Result<AxumJson<Vec<GameListing>>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let games = relay_state
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let now = Utc::now();

    let mut listings: Vec<GameListing> = games
        .values()
        .filter(|g| g.announcement.status == GameStatus::Open)
        .filter(|g| {
            // time_control filter
            let t = g.announcement.base_time_seconds;
            filter.time_min.map_or(true, |mn| t >= mn) && filter.time_max.map_or(true, |mx| t <= mx)
        })
        .filter(|g| {
            let s = g.announcement.stake_amount;
            filter.stake_min.map_or(true, |mn| s >= mn)
                && filter.stake_max.map_or(true, |mx| s <= mx)
        })
        .filter(|g| {
            if filter.elo_min.is_none() && filter.elo_max.is_none() {
                return true;
            }
            let elo = g.announcement.elo.unwrap_or(1200);
            filter.elo_min.map_or(true, |mn| elo >= mn)
                && filter.elo_max.map_or(true, |mx| elo <= mx)
        })
        .map(|g| {
            let elapsed = now.signed_duration_since(g.last_activity).num_seconds();
            let ttl_seconds = (300 - elapsed).max(0);
            GameListing {
                game_id: g.announcement.game_id.clone(),
                display_name: g.announcement.display_name.clone(),
                stake_amount: g.announcement.stake_amount,
                game_type: g.announcement.game_type.clone(),
                base_time_seconds: g.announcement.base_time_seconds,
                increment_seconds: g.announcement.increment_seconds,
                status: g.announcement.status.clone(),
                username: g.announcement.username.clone(),
                elo: g.announcement.elo,
                region: g.announcement.region.clone(),
                capacity: 2,
                players_joined: if g.joiner_node_id.is_some() { 2 } else { 1 },
                ttl_seconds,
                is_private: g.announcement.password_hash.is_some(),
            }
        })
        .collect();

    // Sort
    match filter.sort.as_deref().unwrap_or("newest") {
        "elo_asc" => listings.sort_by_key(|l| l.elo.unwrap_or(0)),
        "elo_desc" => listings.sort_by(|a, b| b.elo.unwrap_or(0).cmp(&a.elo.unwrap_or(0))),
        "stake_asc" => listings.sort_by(|a, b| {
            a.stake_amount
                .partial_cmp(&b.stake_amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "stake_desc" => listings.sort_by(|a, b| {
            b.stake_amount
                .partial_cmp(&a.stake_amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "time_asc" => listings.sort_by_key(|l| l.base_time_seconds),
        _ => listings.sort_by(|a, b| b.ttl_seconds.cmp(&a.ttl_seconds)), // newest first
    }

    Ok(AxumJson(listings))
}

/// Request to join a P2P game
pub async fn join_game(
    State(state): State<AppState>,
    Json(req): Json<JoinGameRequest>,
) -> Result<AxumJson<JoinGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let game_id_clone = req.game_id.clone();
    let joiner_node_id_clone = req.joiner_node_id.clone();

    let Some(game) = games.get_mut(&req.game_id) else {
        return Ok(AxumJson(JoinGameResponse {
            success: false,
            host_node_id: None,
        }));
    };

    if game.announcement.status != GameStatus::Open {
        return Ok(AxumJson(JoinGameResponse {
            success: false,
            host_node_id: None,
        }));
    }

    // Password check for private rooms
    if let Some(ref hash) = game.announcement.password_hash.clone() {
        let provided = req.password.as_deref().unwrap_or("");
        match bcrypt::verify(provided, hash) {
            Ok(true) => {}
            _ => {
                tracing::warn!("Wrong password for game {}", req.game_id);
                return Ok(AxumJson(JoinGameResponse {
                    success: false,
                    host_node_id: None,
                }));
            }
        }
    }

    game.joiner_node_id = Some(req.joiner_node_id);
    game.announcement.status = GameStatus::Connecting;
    game.last_activity = Utc::now();

    let host_node_id = game.announcement.host_node_id.clone();

    tracing::info!(
        "P2P join request: game={}, joiner={}",
        game_id_clone,
        joiner_node_id_clone
    );

    Ok(AxumJson(JoinGameResponse {
        success: true,
        host_node_id: Some(host_node_id),
    }))
}

/// Host accepts the join request and starts the game
pub async fn accept_join(
    State(state): State<AppState>,
    Json(req): Json<AcceptJoinReq>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let game_id_for_log = req.game_id.clone();

    let Some(game) = games.get_mut(&req.game_id) else {
        return Ok(AxumJson(AnnounceGameResponse { success: false }));
    };

    // Verify caller is the host.
    if game.announcement.host_node_id != req.host_node_id {
        return Ok(AxumJson(AnnounceGameResponse { success: false }));
    }

    // Refuse if no joiner has arrived yet — prevents accept racing ahead of join.
    if game.joiner_node_id.is_none() {
        return Ok(AxumJson(AnnounceGameResponse { success: false }));
    }

    game.announcement.status = GameStatus::InProgress;
    game.last_activity = chrono::Utc::now();

    tracing::info!("P2P game {} started", game_id_for_log);

    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

/// Leave a P2P game
pub async fn leave_game(
    State(state): State<AppState>,
    Json(req): Json<LeaveGameRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(game) = games.get_mut(&req.game_id) {
        if game.announcement.host_node_id == req.node_id {
            // Host left - remove game
            game.announcement.status = GameStatus::Finished;
            tracing::info!("P2P game {} ended (host left)", req.game_id);
        } else if game.joiner_node_id.as_ref() == Some(&req.node_id) {
            // Joiner left
            game.joiner_node_id = None;
            game.announcement.status = GameStatus::Open;
            tracing::info!("P2P game {} open again (joiner left)", req.game_id);
        }
    }

    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

/// Send a message in a P2P game
pub async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(game) = games.get_mut(&req.game_id) else {
        return Ok(AxumJson(AnnounceGameResponse { success: false }));
    };

    game.last_activity = chrono::Utc::now();

    if game.announcement.host_node_id == req.from_node_id {
        // Message from host to joiner
        game.host_messages.push(req.message);
    } else if game.joiner_node_id.as_ref() == Some(&req.from_node_id) {
        // Message from joiner to host
        game.joiner_messages.push(req.message);
    } else {
        return Ok(AxumJson(AnnounceGameResponse { success: false }));
    }

    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

/// Poll for new messages in a P2P game
pub async fn poll_messages(
    State(state): State<AppState>,
    Json(req): Json<PollMessagesRequest>,
) -> Result<AxumJson<PollMessagesResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let games = relay_state
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(game) = games.get(&req.game_id) else {
        return Ok(AxumJson(PollMessagesResponse {
            messages: vec![],
            next_index: req.since_index,
        }));
    };

    let messages = if game.announcement.host_node_id == req.node_id {
        // Host polls joiner messages
        game.joiner_messages[req.since_index..].to_vec()
    } else if game.joiner_node_id.as_ref() == Some(&req.node_id) {
        // Joiner polls host messages
        game.host_messages[req.since_index..].to_vec()
    } else {
        vec![]
    };

    let next_index = req.since_index + messages.len();

    Ok(AxumJson(PollMessagesResponse {
        messages,
        next_index,
    }))
}

/// Host heartbeat — refreshes last_activity so the 5-min cleanup doesn't evict a live lobby.
pub async fn heartbeat_game(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(game) = games.get_mut(&req.game_id) {
        if game.announcement.host_node_id == req.host_node_id {
            game.last_activity = Utc::now();
            return Ok(AxumJson(AnnounceGameResponse { success: true }));
        }
    }

    Ok(AxumJson(AnnounceGameResponse { success: false }))
}

/// GET /region — returns the backend's configured region tag
pub async fn get_region() -> AxumJson<serde_json::Value> {
    let region = std::env::var("XFCHESS_REGION").unwrap_or_else(|_| "unknown".to_string());
    let label = match region.as_str() {
        "eu-central" | "eu" => "EU (Frankfurt)",
        "us-east" | "us" => "US East (New York)",
        "us-west" => "US West (Los Angeles)",
        "ap-southeast" => "Asia (Singapore)",
        "ap-northeast" => "Asia (Tokyo)",
        _ => "Unknown Region",
    };
    AxumJson(serde_json::json!({ "region": region, "label": label }))
}
