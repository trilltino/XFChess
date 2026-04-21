# Infrastructure

Server infrastructure and configuration for the XFChess backend, built on Axum and Tokio.

## Overview

The infrastructure layer provides the foundation for the XFChess backend server, handling:
- HTTP server initialization and configuration
- Application state management
- Error handling and middleware
- Configuration loading from environment variables
- Request routing and middleware composition

## Why Axum?

Axum is a modern async web framework for Rust that provides:
- **Type-safe routing** - Compile-time checked route parameters
- **Extractor pattern** - Clean way to access request data
- **Middleware support** - Composable request/response processing
- **Tower ecosystem** - Integrates with Tower middleware ecosystem
- **Performance** - Built on Tokio for high-performance async I/O

## Components

- **HTTP server setup** - Axum server configuration with TLS support
- **State management** - Application state shared across handlers
- **Error handling middleware** - Centralized error response formatting
- **Configuration loading** - Environment variable parsing and validation
- **Middleware composition** - Request logging, CORS, rate limiting

## Application State

The application state is shared across all request handlers using Axum's state extraction:

```rust
use axum::{extract::State, Router};
use std::sync::Arc;

/// Application state shared across all request handlers
/// 
/// This struct contains all the resources that handlers need access to:
/// - Database connection pool
/// - Solana RPC client
/// - Tournament store
/// - Configuration values
/// 
/// Using Arc allows the state to be cloned cheaply and shared across threads.
#[derive(Clone)]
pub struct AppState {
    /// SQLite database connection pool
    /// Used for persistent storage of tournament data
    pub db_pool: SqlitePool,
    
    /// Solana RPC client
    /// Used for blockchain queries and transaction submission
    pub solana_rpc: SolanaRpcClient,
    
    /// In-memory tournament store
    /// Used for fast access to active tournament data
    pub tournament_store: Arc<TournamentStore>,
    
    /// Server configuration
    pub config: ServerConfig,
}

/// Server configuration loaded from environment variables
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Server port to listen on
    pub port: u16,
    /// Solana RPC URL
    pub solana_rpc_url: String,
    /// Database connection string
    pub database_url: String,
    /// Whether to enable debug logging
    pub debug: bool,
}

impl AppState {
    /// Creates a new application state
    /// 
    /// # Arguments
    /// * `config` - Server configuration
    /// 
    /// # Returns
    /// A new AppState instance with all resources initialized
    /// 
    /// # Errors
    /// Returns an error if database connection or RPC client fails to initialize
    pub async fn new(config: ServerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize database connection pool
        let db_pool = SqlitePool::connect(&config.database_url).await?;
        
        // Initialize Solana RPC client
        let solana_rpc = SolanaRpcClient::new(&config.solana_rpc_url).await?;
        
        // Initialize tournament store
        let tournament_store = Arc::new(TournamentStore::new());
        
        Ok(Self {
            db_pool,
            solana_rpc,
            tournament_store,
            config,
        })
    }
}
```

## Example: Creating the Axum App

This example shows how to compose the Axum application with all routes, middleware, and state.

```rust
use axum::{Router, routing::get};
use tower_http::{
    trace::TraceLayer,
    cors::{CorsLayer, Any},
    compression::CompressionLayer,
};
use tracing::Level;

/// Creates the Axum application with all routes and middleware
/// 
/// This function:
/// 1. Initializes the application state
/// 2. Adds all route handlers
/// 3. Configures middleware (tracing, CORS, compression)
/// 4. Returns the configured Router
/// 
/// # Returns
/// A configured Axum Router ready to serve requests
/// 
/// # Errors
/// Returns an error if application state initialization fails
pub async fn create_app() -> Result<Router, Box<dyn std::error::Error>> {
    // Load configuration from environment
    let config = load_config_from_env()?;
    
    // Initialize application state
    let state = AppState::new(config).await?;
    
    // Build the router with all routes
    let app = Router::new()
        // Health check endpoint (no state required)
        .route("/health", get(health_check))
        
        // Tournament routes
        .route("/tournaments", get(list_tournaments))
        .route("/tournament/:id", get(get_tournament))
        .route("/tournament/:id/join", post(join_tournament))
        
        // Blinks routes
        .route("/api/actions/tournament/:id", get(get_blinks_action))
        .route("/api/actions/tournament/:id/register", post(build_registration_tx))
        
        // Signing routes
        .route("/signing/build", post(build_transaction))
        .route("/signing/sign", post(sign_transaction))
        
        // Add application state to all routes
        .with_state(state)
        
        // Add middleware layers
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                    )
                })
                .on_response(|response: &axum::http::Response<_>, latency: Duration, span: &tracing::Span| {
                    span.record("latency_ms", latency.as_millis());
                    tracing::info!(
                        "status = %s, latency_ms = {}",
                        response.status(),
                        latency.as_millis()
                    );
                })
        )
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(CompressionLayer::new());
    
    Ok(app)
}

/// Health check endpoint
/// 
/// Returns a simple status response to indicate the server is running.
/// This is useful for load balancers and monitoring systems.
/// 
/// # Returns
/// A JSON response with server status
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
```

## Example: State Management

Axum's state extraction allows handlers to access application state cleanly:

```rust
use axum::{extract::State, Json};
use serde_json::Value;

/// Handler that uses application state
/// 
/// The `State` extractor automatically provides the application state
/// to the handler. This is type-safe and handled by Axum at compile time.
/// 
/// # Arguments
/// * `State(state)` - The application state
/// 
/// # Returns
/// A JSON response with tournament data
async fn list_tournaments(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, AppError> {
    // Access the database pool from state
    let tournaments = db::list_tournaments(&state.db_pool, None).await?;
    
    Ok(Json(serde_json::json!({
        "tournaments": tournaments,
        "count": tournaments.len(),
    })))
}

/// Handler that modifies application state
/// 
/// This handler demonstrates how to use mutable state when needed.
/// Note that the state is wrapped in Arc, so it can be shared across threads.
/// 
/// # Arguments
/// * `State(state)` - The application state
/// * `Json(payload)` - The request body
/// 
/// # Returns
/// A JSON response with the created tournament
async fn create_tournament(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTournamentRequest>,
) -> Result<Json<Value>, AppError> {
    // Access the tournament store from state
    let tournament_id = state.tournament_store
        .create_tournament(payload.name, payload.entry_fee)
        .await?;
    
    // Also persist to database
    db::create_tournament(&state.db_pool, &payload.name, payload.entry_fee, 0).await?;
    
    Ok(Json(serde_json::json!({
        "id": tournament_id,
        "name": payload.name,
        "status": "created",
    })))
}
```

## Example: Error Handling

Centralized error handling ensures consistent error responses across all endpoints:

```rust
use axum::{response::IntoResponse, http::StatusCode, Json};
use thiserror::Error;

/// Application error type
/// 
/// All errors in the application should convert to this type for consistent
/// error responses. The `IntoResponse` trait implementation handles
/// converting errors to HTTP responses with appropriate status codes.
#[derive(Error, Debug)]
pub enum AppError {
    /// Database error from SQLx
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    /// Solana RPC error
    #[error("Solana RPC error: {0}")]
    SolanaRpc(#[from] SolanaRpcError),
    
    /// Resource not found
    #[error("Resource not found")]
    NotFound,
    
    /// Unauthorized access
    #[error("Unauthorized")]
    Unauthorized,
    
    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    BadRequest(String),
    
    /// Internal server error
    #[error("Internal server error: {0}")]
    Internal(String),
}

/// Converts AppError to HTTP response
/// 
/// This implementation ensures all errors are returned as JSON responses
/// with appropriate status codes and error messages.
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
            }
            AppError::SolanaRpc(e) => {
                tracing::error!("Solana RPC error: {}", e);
                (StatusCode::BAD_GATEWAY, format!("Solana error: {}", e))
            }
            AppError::NotFound => (StatusCode::NOT_FOUND, "Resource not found".to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };
        
        let body = serde_json::json!({
            "error": true,
            "message": message,
            "status": status.as_u16(),
        });
        
        (status, Json(body)).into_response()
    }
}

/// Helper to convert Result to AppError
/// 
/// This trait allows easy conversion of any error type to AppError.
pub trait IntoAppError<T> {
    fn into_app_error(self) -> Result<T, AppError>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> IntoAppError<T> for Result<T, E> {
    fn into_app_error(self) -> Result<T, AppError> {
        self.map_err(|e| AppError::Internal(e.to_string()))
    }
}
```

## Example: Configuration Loading

Configuration is loaded from environment variables with validation:

```rust
use std::env;

/// Loads server configuration from environment variables
/// 
/// This function reads configuration from environment variables and validates
/// that all required values are present. Missing values will cause an error.
/// 
/// # Environment Variables
/// - `PORT` - Server port (default: 3000)
/// - `SOLANA_RPC_URL` - Solana RPC endpoint (required)
/// - `DATABASE_URL` - Database connection string (required)
/// - `DEBUG` - Enable debug logging (default: false)
/// 
/// # Returns
/// A ServerConfig struct with all configuration values
/// 
/// # Errors
/// Returns an error if required environment variables are missing
pub fn load_config_from_env() -> Result<ServerConfig, ConfigError> {
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .map_err(|_| ConfigError::InvalidPort)?;
    
    let solana_rpc_url = env::var("SOLANA_RPC_URL")
        .map_err(|_| ConfigError::MissingSolanaRpcUrl)?;
    
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| ConfigError::MissingDatabaseUrl)?;
    
    let debug = env::var("DEBUG")
        .unwrap_or_else(|_| "false".to_string())
        .parse()
        .unwrap_or(false);
    
    Ok(ServerConfig {
        port,
        solana_rpc_url,
        database_url,
        debug,
    })
}

/// Configuration error types
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid PORT value")]
    InvalidPort,
    
    #[error("Missing SOLANA_RPC_URL environment variable")]
    MissingSolanaRpcUrl,
    
    #[error("Missing DATABASE_URL environment variable")]
    MissingDatabaseUrl,
}
```

## Example: Middleware Composition

Axum uses Tower for middleware, allowing composable request/response processing:

```rust
use tower::{ServiceBuilder, ServiceExt};
use tower_http::{
    trace::TraceLayer,
    cors::CorsLayer,
    compression::CompressionLayer,
    limit::RequestBodyLimitLayer,
};

/// Creates a custom middleware stack
/// 
/// This example shows how to compose multiple middleware layers:
/// 1. Request body size limiting
/// 2. Compression
/// 3. CORS
/// 4. Request tracing
/// 
/// # Returns
/// A configured middleware stack
pub fn create_middleware_stack() -> tower::ServiceBuilder<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>> {
    ServiceBuilder::new()
        // Limit request body size to 10MB
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
        
        // Compress responses with gzip
        .layer(CompressionLayer::new())
        
        // Allow CORS from any origin (configure appropriately for production)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        )
        
        // Trace all requests
        .layer(TraceLayer::new_for_http())
}
```

## Example: Creating the Axum App

```rust
use axum::{Router, routing::get};
use backend::signing::routes;

pub async fn create_app() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .nest("/api", routes::router())
        .layer(tower_http::trace::TraceLayer::new_for_http())
}
```

## Example: State Management

```rust
use axum::{extract::State, Router};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    db_pool: SqlitePool,
}

async fn handler(State(state): State<Arc<AppState>>) -> String {
    "OK".to_string()
}

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/test", get(handler))
        .with_state(state)
}
```

## Example: Error Handling

```rust
use axum::{response::IntoResponse, http::StatusCode};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Not found")]
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "Resource not found".to_string()),
        };
        
        (status, message).into_response()
    }
}
```
