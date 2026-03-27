use axum::{
    extract::Json,
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::net::TcpListener;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveRequest {
    pub fen: String,
    pub player_side: String, // "white" or "black"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveResponse {
    pub best_move: String,
    pub evaluation: i32,
    pub depth: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

// Simple AI service - placeholder for Stockfish integration
fn get_best_move(fen: &str, _player_side: &str) -> MoveResponse {
    // This would integrate with Stockfish in a real implementation
    // For now, return a placeholder response
    MoveResponse {
        best_move: "e2e4".to_string(), // Placeholder move
        evaluation: 0,
        depth: 15,
    }
}

async fn health() -> ResponseJson<HealthResponse> {
    ResponseJson(HealthResponse {
        status: "healthy".to_string(),
        version: "0.1.0".to_string(),
    })
}

async fn get_move(Json(request): Json<MoveRequest>) -> Result<ResponseJson<MoveResponse>, StatusCode> {
    info!("Received move request for FEN: {}", request.fen);
    
    let response = get_best_move(&request.fen, &request.player_side);
    
    Ok(ResponseJson(response))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("xfchess_ai_service=debug,tower_http=debug")
        .init();

    info!("Starting XFChess AI Service");

    // Create router
    let app = Router::new()
        .route("/health", get(health))
        .route("/move", post(get_move))
        .route("/", get(|| async { ResponseJson(HealthResponse { status: "XFChess AI Service", version: "0.1.0".to_string() }) }));

    // Bind to port
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    info!("XFChess AI Service listening on 0.0.0.0:8080");

    // Run server
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let response = health().await;
        assert_eq!(response.status, "healthy");
    }

    #[test]
    fn test_get_best_move() {
        let request = MoveRequest {
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            player_side: "white".to_string(),
        };

        let response = get_best_move(&request.fen, &request.player_side);
        assert!(!response.best_move.is_empty());
        assert_eq!(response.depth, 15);
    }
}
