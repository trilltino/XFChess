//! HTTP route handlers for P2P relay.

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as AxumJson,
    routing::{get, post},
    Router,
};

use crate::signing::AppState;

use super::types::{
    ActiveGame, AnnounceGameRequest, AnnounceGameResponse, GameListing, GameStatus,
    JoinGameRequest, JoinGameResponse, LeaveGameRequest, PollMessagesRequest,
    PollMessagesResponse, SendMessageRequest,
};

/// Creates the P2P relay router
pub fn p2p_routes() -> Router<AppState> {
    Router::new()
        .route("/p2p/announce", post(announce_game))
        .route("/p2p/games", get(list_games))
        .route("/p2p/join", post(join_game))
        .route("/p2p/accept", post(accept_join))
        .route("/p2p/leave", post(leave_game))
        .route("/p2p/message", post(send_message))
        .route("/p2p/poll", post(poll_messages))
}

/// Announce a new P2P game
pub async fn announce_game(
    State(state): State<AppState>,
    Json(req): Json<AnnounceGameRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let game_id_clone = req.game_id.clone();
    let host_node_id_clone = req.host_node_id.clone();

    let announcement = super::types::P2PGameAnnouncement {
        game_id: req.game_id,
        host_node_id: req.host_node_id,
        display_name: req.display_name,
        stake_amount: req.stake_amount,
        game_type: req.game_type,
        time_control_minutes: req.time_control_minutes,
        created_at: chrono::Utc::now(),
        status: GameStatus::Open,
    };

    let active_game = ActiveGame {
        announcement,
        joiner_node_id: None,
        host_messages: Vec::new(),
        joiner_messages: Vec::new(),
        last_activity: chrono::Utc::now(),
    };

    games.insert(game_id_clone.clone(), active_game);

    tracing::info!("P2P game announced: {} by {}", game_id_clone, host_node_id_clone);

    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

/// List all open P2P games
pub async fn list_games(
    State(state): State<AppState>,
) -> Result<AxumJson<Vec<GameListing>>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let games = relay_state.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let listings: Vec<GameListing> = games
        .values()
        .filter(|g| g.announcement.status == GameStatus::Open)
        .map(|g| GameListing {
            game_id: g.announcement.game_id.clone(),
            display_name: g.announcement.display_name.clone(),
            stake_amount: g.announcement.stake_amount,
            game_type: g.announcement.game_type.clone(),
            time_control_minutes: g.announcement.time_control_minutes,
            status: g.announcement.status.clone(),
        })
        .collect();

    Ok(AxumJson(listings))
}

/// Request to join a P2P game
pub async fn join_game(
    State(state): State<AppState>,
    Json(req): Json<JoinGameRequest>,
) -> Result<AxumJson<JoinGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let game_id_clone = req.game_id.clone();
    let joiner_node_id_clone = req.joiner_node_id.clone();

    let Some(game) = games.get_mut(&req.game_id) else {
        return Ok(AxumJson(JoinGameResponse { success: false, host_node_id: None }));
    };

    if game.announcement.status != GameStatus::Open {
        return Ok(AxumJson(JoinGameResponse { success: false, host_node_id: None }));
    }

    game.joiner_node_id = Some(req.joiner_node_id);
    game.announcement.status = GameStatus::Connecting;
    game.last_activity = chrono::Utc::now();

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
    Json(req): Json<JoinGameRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let game_id_for_log = req.game_id.clone();

    let Some(game) = games.get_mut(&req.game_id) else {
        return Ok(AxumJson(AnnounceGameResponse { success: false }));
    };

    if game.announcement.host_node_id != req.joiner_node_id {
        // Wrong host trying to accept
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
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
    let games = relay_state.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
