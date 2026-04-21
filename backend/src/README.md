# Backend Source Code

This directory contains the backend server source code for XFChess, providing HTTP APIs, Solana transaction signing, and tournament management services.

## Architecture Overview

The XFChess backend is built with Rust and uses:
- **Axum** - Modern async web framework for HTTP APIs
- **SQLx** - Async SQL toolkit for database operations
- **Solana SDK** - Blockchain integration for transaction building and signing
- **Tokio** - Async runtime for concurrent operations

The backend serves as the bridge between the game client and the Solana blockchain, handling transaction signing, tournament state management, and providing APIs for tournament discovery and registration.

## Directory Structure

- **signing/** - Transaction signing and Solana integration
  - **blinks.rs** - Solana Blinks API implementation for action metadata
  - **blinks_pda.rs** - PDA (Program Derived Address) calculations
  - **blinks_anti_cheat.rs** - IP-based anti-cheat mechanisms
  - **blinks_chains.rs** - Action chaining for multi-step flows
  - **blinks_onboarding.rs** - User onboarding state machine
  - **blinks_funding.rs** - Integration with MoonPay/Transak/Banxa
  - **routes/** - HTTP route handlers for signing endpoints
  - **tournament_store.rs** - In-memory tournament state management
  - **cacf/** - Country-specific compliance (UK, Brazil, Germany, Canada)
  - **p2p_relay/** - P2P relay service for multiplayer
  - **solana/** - Solana transaction building and RPC
  - **mod.rs** - Module exports and initialization

- **db/** - Database layer and models
  - SQLite database connection management
  - Migration system
  - Tournament and game data models

- **infrastructure/** - Server infrastructure and configuration
  - HTTP server setup with Axum
  - State management
  - Error handling middleware
  - Configuration loading from environment

- **lib.rs** - Main library module exports and public API

## API Endpoints

### Tournament Endpoints
- `GET /tournaments` - List all active tournaments
- `GET /tournament/{id}` - Get tournament details
- `POST /tournament/{id}/join` - Join a tournament
- `GET /tournament/{id}/my-match` - Get current match for player
- `GET /tournament/{id}/bracket` - Get tournament bracket

### Blinks Endpoints
- `GET /api/actions/tournament/{id}` - Get action metadata
- `POST /api/actions/tournament/{id}/register` - Build registration transaction
- `GET /api/actions/tournament/{id}/check-balance` - Check SOL balance
- `POST /api/actions/tournament/{id}/validate` - Anti-cheat validation
- `GET /api/actions/tournament/{id}/chain/registration` - Registration action chain
- `GET /api/actions/tournament/{id}/chain/onboarding` - Onboarding action chain

### Signing Endpoints
- `POST /signing/build` - Build a Solana transaction
- `POST /signing/sign` - Sign a transaction
- `POST /signing/submit` - Submit a transaction to Solana

## Example: Starting the Backend Server

```rust
use axum::Router;
use backend::infrastructure::create_app;

/// Main entry point for the XFChess backend server
/// 
/// This function initializes the async runtime, creates the Axum application,
/// binds to a TCP listener, and serves HTTP requests.
/// 
/// # Environment Variables
/// - `DATABASE_URL` - SQLite database connection string
/// - `SOLANA_RPC_URL` - Solana RPC endpoint
/// - `PORT` - Server port (default: 3000)
#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::init();
    
    // Create the Axum application with all routes and middleware
    let app = create_app().await;
    
    // Bind to TCP listener on all interfaces
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .unwrap();
    
    // Start serving HTTP requests
    tracing::info!("Backend server listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
```

## Example: Adding a New Route

Routes in Axum are defined using the Router builder pattern. Each route handler is an async function that receives request data and returns a response.

```rust
use axum::{Json, Router, response::IntoResponse};
use axum::routing::post;
use serde::{Deserialize, Serialize};

/// Request structure for the custom endpoint
#[derive(Deserialize)]
struct Request {
    data: String,
}

/// Response structure for the custom endpoint
#[derive(Serialize)]
struct Response {
    data: String,
    status: String,
}

/// Async handler function for the custom endpoint
/// 
/// # Arguments
/// * `Json(payload)` - Deserialized JSON request body
/// 
/// # Returns
/// A JSON response with the processed data
async fn custom_handler(Json(payload): Json<Request>) -> impl IntoResponse {
    Json(Response {
        data: payload.data,
        status: "success".to_string(),
    })
}

/// Router function that defines all routes for this module
/// 
/// # Returns
/// An Axum Router configured with all routes
fn router() -> Router {
    Router::new()
        .route("/api/custom", post(custom_handler))
}
```

## Example: Database Query with SQLx

SQLx provides compile-time checked SQL queries and async database operations.

```rust
use sqlx::{SqlitePool, FromRow};
use serde::Serialize;

/// Tournament model representing a row in the tournaments table
#[derive(Debug, Serialize, FromRow)]
struct Tournament {
    id: i64,
    name: String,
    entry_fee: u64,
    prize_pool: u64,
    status: String,
    player_count: i32,
    created_at: i64,
}

/// Retrieve all tournaments from the database
/// 
/// # Arguments
/// * `pool` - SQLite database connection pool
/// 
/// # Returns
/// A vector of Tournament structs representing all tournaments
/// 
/// # Errors
/// Returns a sqlx::Error if the query fails
async fn get_tournaments(pool: &SqlitePool) -> Result<Vec<Tournament>, sqlx::Error> {
    sqlx::query_as::<_, Tournament>("SELECT * FROM tournaments ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
}

/// Retrieve a specific tournament by ID
/// 
/// # Arguments
/// * `pool` - SQLite database connection pool
/// * `id` - Tournament ID to retrieve
/// 
/// # Returns
/// The Tournament struct if found, or None if not found
/// 
/// # Errors
/// Returns a sqlx::Error if the query fails
async fn get_tournament_by_id(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<Tournament>, sqlx::Error> {
    sqlx::query_as::<_, Tournament>("SELECT * FROM tournaments WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Create a new tournament in the database
/// 
/// # Arguments
/// * `pool` - SQLite database connection pool
/// * `name` - Tournament name
/// * `entry_fee` - Entry fee in lamports
/// * `prize_pool` - Total prize pool in lamports
/// 
/// # Returns
/// The ID of the newly created tournament
/// 
/// # Errors
/// Returns a sqlx::Error if the query fails
async fn create_tournament(
    pool: &SqlitePool,
    name: &str,
    entry_fee: u64,
    prize_pool: u64,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO tournaments (name, entry_fee, prize_pool, status, player_count, created_at) 
         VALUES (?, ?, ?, 'open', 0, ?)"
    )
    .bind(name)
    .bind(entry_fee)
    .bind(prize_pool)
    .bind(chrono::Utc::now().timestamp())
    .execute(pool)
    .await?;
    
    Ok(result.last_insert_rowid())
}
```

## Solana Transaction Building

The backend builds Solana transactions for tournament operations:

```rust
use solana_sdk::{transaction::Transaction, pubkey::Pubkey, instruction::Instruction};
use solana_sdk::signature::Keypair;

/// Build a Solana transaction for tournament registration
/// 
/// # Arguments
/// * `program_id` - The XFChess program ID on Solana
/// * `payer` - The account that will pay for the transaction
/// * `tournament_id` - The tournament to register for
/// * `player_pubkey` - The player's public key
/// 
/// # Returns
/// A signed Transaction ready for submission
/// 
/// # Errors
/// Returns an error if transaction building fails
async fn build_registration_transaction(
    program_id: Pubkey,
    payer: &Keypair,
    tournament_id: u64,
    player_pubkey: Pubkey,
) -> Result<Transaction, Box<dyn std::error::Error>> {
    // Create the instruction data
    let instruction_data = serialize_instruction_data(tournament_id, player_pubkey)?;
    
    // Calculate tournament PDA
    let (tournament_pda, _) = Pubkey::find_program_address(
        &[b"tournament", &tournament_id.to_le_bytes()],
        &program_id,
    );
    
    // Build the instruction
    let instruction = Instruction::new_with_bytes(
        program_id,
        &instruction_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
    );
    
    // Create the transaction
    let transaction = Transaction::new(
        &[instruction],
        Some(&payer.pubkey()),
    );
    
    Ok(transaction)
}

/// Serialize instruction data for tournament registration
/// 
/// # Arguments
/// * `tournament_id` - Tournament ID
/// * `player_pubkey` - Player's public key
/// 
/// # Returns
/// Serialized instruction data as bytes
fn serialize_instruction_data(
    tournament_id: u64,
    player_pubkey: Pubkey,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut data = Vec::new();
    data.extend_from_slice(&0u8.to_le_bytes()); // Instruction discriminator
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(player_pubkey.as_ref());
    Ok(data)
}
```
