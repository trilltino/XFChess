use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber;

use braid_iroh::{Node, NodeId};
use iroh_gossip::net::Gossip;
use shared::{GameStateMessage, BoardState};

// Struct to hold our application state
#[derive(Clone)]
struct AppState {
    /// The Iroh node for P2P networking
    node: Arc<Node>,
    /// Gossip topic for XFChess
    gossip_topic: Arc<Gossip>,
    /// Indexed game states
    indexed_games: Arc<RwLock<Vec<GameRecord>>>,
    /// Active observers tracking ongoing games
    active_observers: Arc<RwLock<Vec<NodeId>>>,
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

    // Create application state
    let app_state = AppState {
        node: Arc::new(node),
        gossip_topic: Arc::new(topic.clone()),
        indexed_games: Arc::new(RwLock::new(Vec::new())),
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
                            let game_record = GameRecord {
                                id: game_msg.game_id.clone(),
                                players: [game_msg.player1.unwrap_or_default(), game_msg.player2.unwrap_or_default()],
                                stake_amount: game_msg.stake_amount.unwrap_or(0.0),
                                start_time: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                                moves: vec![],
                                end_time: None,
                                winner: None,
                            };
                            
                            state.indexed_games.write().await.push(game_record);
                            tracing::info!("Indexed new game: {}", game_msg.game_id);
                        }
                        "move_made" => {
                            // Find the game and add the move
                            let mut games = state.indexed_games.write().await;
                            if let Some(game) = games.iter_mut().find(|g| g.id == game_msg.game_id) {
                                if let Some(movement) = game_msg.move_details {
                                    game.moves.push(movement);
                                    tracing::info!("Recorded move for game {}: {}", game_msg.game_id, movement);
                                }
                            }
                        }
                        "game_end" => {
                            // Find the game and mark it as ended
                            let mut games = state.indexed_games.write().await;
                            if let Some(game) = games.iter_mut().find(|g| g.id == game_msg.game_id) {
                                game.end_time = Some(
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs()
                                );
                                
                                if let Some(winner) = game_msg.winner {
                                    game.winner = Some(winner);
                                }
                                
                                tracing::info!("Game {} ended. Winner: {:?}", game_msg.game_id, game.winner);
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
async fn health_handler(State(_state): State<AppState>) -> &'static str {
    "OK"
}

// Handler to list all indexed games
async fn list_games_handler(State(state): State<AppState>) -> Json<Vec<GameRecord>> {
    let games = state.indexed_games.read().await.clone();
    Json(games)
}

// Handler to get a specific game by ID
async fn get_game_handler(Path(game_id): Path<String>, State(state): State<AppState>) -> Result<Json<GameRecord>, StatusCode> {
    let games = state.indexed_games.read().await;
    let game = games.iter().find(|g| g.id == game_id);
    
    match game {
        Some(game) => Ok(Json(game.clone())),
        None => Err(StatusCode::NOT_FOUND),
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