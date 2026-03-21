use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing_subscriber;

use braid_iroh::{Node, NodeId};
use iroh_gossip::net::Gossip;
use shared::{GameStateMessage, BoardState};
use backend::db::{GameRepository, GameRecord, MoveRecord, GameStats};

// Struct to hold our application state
#[derive(Clone)]
struct AppState {
    /// The Iroh node for P2P networking
    node: Arc<Node>,
    /// Gossip topic for XFChess
    gossip_topic: Arc<Gossip>,
    /// Database connection pool
    db_pool: SqlitePool,
    /// Game repository for database operations
    game_repo: Arc<GameRepository>,
    /// Active observers tracking ongoing games
    active_observers: Arc<RwLock<Vec<NodeId>>>,
    indexed_games: Arc<RwLock<Vec<GameRecord>>>,
    /// New field: game_index
    game_index: Arc<RwLock<HashMap<String, GameRecord>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameRecord {
    id: String,
    players: [String; 2], // Player node IDs
    stake_amount: f64,
    start_time: u64,
    moves: Vec<String>,
    end_time: Option<u64>,
    winner: Option<String>,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Starting XFChess Braid Sidecar Node...");

    // Initialize the Iroh node
    let node = Node::builder().spawn().await.unwrap();
    let node_id = node.node_id();
    
    println!("Braid Sidecar initialized with Node ID: {}", node_id);

    // Join the XFChess gossip topic to observe games
    let gossip = node.gossip();
    let topic = gossip.topic(b"XFChess-0.5-SOL".to_vec()).await.unwrap();
    
    println!("Joined XFChess gossip topic");

    // Initialize database
    let db_url = std::env::var("XFCHESS_DB_URL").unwrap_or_else(|_| "sqlite:xfchess.db".to_string());
    let db_pool = backend::db::init_db(&db_url).await.expect("Failed to initialize database");
    let game_repo = Arc::new(GameRepository::new(db_pool.clone()));
    
    println!("Database initialized: {}", db_url);

    // Create application state
    let app_state = AppState {
        node: Arc::new(node),
        gossip_topic: Arc::new(topic.clone()),
        db_pool,
        game_repo,
        active_observers: Arc::new(RwLock::new(Vec::new())),
    };

    // Start observing the network
    let state_clone = app_state.clone();
    tokio::spawn(async move {
        observe_network(state_clone).await;
    });

    // Build our application with the registered handlers
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/games", get(list_games_handler))
        .route("/games/:id", get(get_game_handler))
        .route("/games/:id/moves", get(get_game_moves_handler))
        .route("/stats", get(get_stats_handler))
        .route("/observe", post(start_observing_handler))
        .with_state(app_state);

    // Run the server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Braid Sidecar server running on http://0.0.0.0:3000");
    
    axum::serve(listener, app)
        .await
        .unwrap();
}

/// Observes the network for new games and indexes them
async fn observe_network(state: AppState) {
    let mut stream = state.gossip_topic.subscribe().await.unwrap();
    
    while let Some(event) = stream.next().await {
        match event {
            iroh_gossip::net::Event::Broadcast { origin, data } => {
                // Check if this is game-related data
                if let Ok(game_msg) = serde_json::from_slice::<GameStateMessage>(&data) {
                    match game_msg.message_type.as_str() {
                        "game_start" => {
                            if let (Some(player1), Some(player2)) = (game_msg.player1, game_msg.player2) {
                                match state.game_repo.create_game(
                                    &game_msg.game_id,
                                    &player1,
                                    &player2,
                                    game_msg.stake_amount.unwrap_or(0.0),
                                ).await {
                                    Ok(_) => tracing::info!("Created game in DB: {}", game_msg.game_id),
                                    Err(e) => tracing::error!("Failed to create game: {}", e),
                                }
                            }
                        }
                        "move_made" => {
                            if let Some(move_uci) = game_msg.move_details {
                                // Get current move count for this game
                                if let Ok(moves) = state.game_repo.get_moves(&game_msg.game_id).await {
                                    let move_number = moves.len() as i32 + 1;
                                    
                                    match state.game_repo.add_move(
                                        &game_msg.game_id,
                                        move_number,
                                        &move_uci,
                                        None, // SAN notation
                                        None, // FEN before
                                        None, // FEN after
                                        "unknown", // player - could be extracted from message
                                    ).await {
                                        Ok(_) => tracing::info!("Recorded move for game {}: {}", game_msg.game_id, move_uci),
                                        Err(e) => tracing::error!("Failed to record move: {}", e),
                                    }
                                }
                            }
                        }
                        "game_end" => {
                            match state.game_repo.end_game(
                                &game_msg.game_id,
                                game_msg.winner.as_deref(),
                                None, // final_fen
                                "completed",
                            ).await {
                                Ok(_) => tracing::info!("Game {} ended. Winner: {:?}", game_msg.game_id, game_msg.winner),
                                Err(e) => tracing::error!("Failed to end game: {}", e),
                            }
                        }
                        _ => {
                            tracing::debug!("Received other gossip message from {}: {:?}", origin, game_msg.message_type);
                        }
                    }
                }
            }
            iroh_gossip::net::Event::NeighborUp(node_id) => {
                tracing::info!("New peer discovered: {}", node_id);
            }
            iroh_gossip::net::Event::NeighborDown(node_id) => {
                tracing::info!("Peer disconnected: {}", node_id);
            }
            _ => {}
        }
    }
}

// Health check handler
async fn health_handler(State(state): State<AppState>) -> Result<Json<HealthStatus>, StatusCode> {
    // Check database connectivity
    let db_healthy = sqlx::query("SELECT 1")
        .fetch_one(&state.db_pool)
        .await
        .is_ok();
    
    let status = HealthStatus {
        status: if db_healthy { "healthy" } else { "unhealthy" },
        database: db_healthy,
        node_id: state.node.node_id().to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    
    Ok(Json(status))
}

#[derive(Serialize)]
struct HealthStatus {
    status: &'static str,
    database: bool,
    node_id: String,
    timestamp: u64,
}

// Handler to list all indexed games
async fn list_games_handler(State(state): State<AppState>) -> Result<Json<Vec<GameRecord>>, StatusCode> {
    match state.game_repo.list_games(None, None).await {
        Ok(games) => Ok(Json(games)),
        Err(e) => {
            tracing::error!("Failed to list games: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Handler to get a specific game by ID
async fn get_game_handler(Path(game_id): Path<String>, State(state): State<AppState>) -> Result<Json<GameRecord>, StatusCode> {
    match state.game_repo.get_game(&game_id).await {
        Ok(Some(game)) => Ok(Json(game)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get game {}: {}", game_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Handler to get all moves for a specific game
async fn get_game_moves_handler(Path(game_id): Path<String>, State(state): State<AppState>) -> Result<Json<Vec<MoveRecord>>, StatusCode> {
    match state.game_repo.get_moves(&game_id).await {
        Ok(moves) => Ok(Json(moves)),
        Err(e) => {
            tracing::error!("Failed to get moves for game {}: {}", game_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Handler to get game statistics
async fn get_stats_handler(State(state): State<AppState>) -> Result<Json<GameStats>, StatusCode> {
    match state.game_repo.get_stats().await {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => {
            tracing::error!("Failed to get stats: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Handler to start observing a specific game
async fn start_observing_handler(
    State(state): State<AppState>,
    Json(payload): Json<ObserveRequest>,
) -> Result<&'static str, StatusCode> {
    // Add the observer to the list
    let mut observers = state.active_observers.write().await;
    if !observers.contains(&payload.observer_node_id) {
        observers.push(payload.observer_node_id);
        Ok("Observation started")
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

#[derive(Deserialize)]
struct ObserveRequest {
    observer_node_id: NodeId,
    game_id: String,
}