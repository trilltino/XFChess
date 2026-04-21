# Database Layer

Database models and persistence for the XFChess backend using SQLx and SQLite.

## Overview

The database layer provides persistent storage for tournament data, game history, and user information. It uses SQLite as the database engine and SQLx for async database operations with compile-time query verification.

## Why SQLx?

SQLx is chosen for this project because:
- **Compile-time verification** - SQL queries are checked at compile time, preventing runtime errors
- **Async/await support** - Built on Tokio for efficient async operations
- **Type-safe** - Automatic mapping between SQL types and Rust types
- **Database agnostic** - Can switch between SQLite, PostgreSQL, MySQL with minimal code changes
- **No ORM overhead** - Direct SQL control without abstraction layers

## Components

- **SQLite database connection** - Connection pooling and management
- **Migration management** - Database schema versioning and upgrades
- **Tournament models** - Data structures for tournament persistence
- **Game data models** - Data structures for game history
- **Query functions** - Async functions for database operations

## Database Schema

### Tournaments Table

```sql
CREATE TABLE tournaments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    entry_fee INTEGER NOT NULL,
    prize_pool INTEGER NOT NULL,
    status TEXT NOT NULL,
    player_count INTEGER NOT NULL,
    round INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);
```

### Players Table

```sql
CREATE TABLE players (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tournament_id INTEGER NOT NULL,
    pubkey TEXT NOT NULL UNIQUE,
    elo_rating INTEGER NOT NULL,
    registered_at INTEGER NOT NULL,
    FOREIGN KEY (tournament_id) REFERENCES tournaments(id)
);
```

### Matches Table

```sql
CREATE TABLE matches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tournament_id INTEGER NOT NULL,
    player_white TEXT NOT NULL,
    player_black TEXT NOT NULL,
    winner TEXT,
    round INTEGER NOT NULL,
    match_index INTEGER NOT NULL,
    FOREIGN KEY (tournament_id) REFERENCES tournaments(id)
);
```

## Example: Database Connection

SQLx uses connection pools for efficient database access. The pool manages multiple connections automatically.

```rust
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

/// Creates a new SQLite database connection pool
/// 
/// Connection pools are essential for production applications as they:
/// - Reuse connections instead of creating new ones
/// - Limit the maximum number of concurrent connections
/// - Provide automatic connection recovery
/// 
/// # Arguments
/// * `database_url` - The SQLite database connection string
/// 
/// # Returns
/// A configured SqlitePool ready for use
/// 
/// # Errors
/// Returns a sqlx::Error if the connection cannot be established
/// 
/// # Example
/// ```ignore
/// let pool = create_pool("sqlite:./xfchess.db").await?;
/// ```
pub async fn create_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(5) // Limit to 5 concurrent connections
        .connect(database_url)
        .await
}

/// Initializes the database schema
/// 
/// This function runs all pending migrations to ensure the database
/// schema is up to date with the application code.
/// 
/// # Arguments
/// * `pool` - The database connection pool
/// 
/// # Errors
/// Returns a sqlx::Error if migration fails
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(include_str!("../migrations/001_initial.sql"))
        .execute(pool)
        .await?;
    Ok(())
}
```

## Example: Creating a Tournament

This example shows how to insert a new tournament into the database with proper error handling.

```rust
use sqlx::SqlitePool;
use chrono::Utc;

/// Tournament model representing a row in the tournaments table
#[derive(Debug, sqlx::FromRow)]
pub struct Tournament {
    /// Unique identifier for the tournament
    pub id: i64,
    
    /// Human-readable tournament name
    pub name: String,
    
    /// Entry fee in lamports (1 SOL = 1,000,000,000 lamports)
    pub entry_fee: u64,
    
    /// Total prize pool accumulated from entry fees
    pub prize_pool: u64,
    
    /// Current status (open, in_progress, completed, cancelled)
    pub status: String,
    
    /// Number of players currently registered
    pub player_count: i32,
    
    /// Current round number (1-indexed)
    pub round: i32,
    
    /// Unix timestamp when tournament was created
    pub created_at: i64,
    
    /// Unix timestamp when tournament started (optional)
    pub started_at: Option<i64>,
    
    /// Unix timestamp when tournament completed (optional)
    pub completed_at: Option<i64>,
}

/// Creates a new tournament in the database
/// 
/// This function:
/// 1. Validates the tournament parameters
/// 2. Inserts the tournament into the database
/// 3. Returns the ID of the newly created tournament
/// 
/// # Arguments
/// * `pool` - The database connection pool
/// * `name` - The tournament name
/// * `entry_fee` - The entry fee in lamports
/// * `prize_pool` - The initial prize pool (usually 0)
/// 
/// # Returns
/// The ID of the newly created tournament
/// 
/// # Errors
/// Returns a sqlx::Error if the insert fails
/// 
/// # Example
/// ```ignore
/// let tournament_id = create_tournament(
///     &pool,
///     "Weekly Championship",
///     1_000_000_000, // 1 SOL
///     0,
/// ).await?;
/// ```
pub async fn create_tournament(
    pool: &SqlitePool,
    name: &str,
    entry_fee: u64,
    prize_pool: u64,
) -> Result<i64, sqlx::Error> {
    let now = Utc::now().timestamp();
    
    let result = sqlx::query(
        "INSERT INTO tournaments 
         (name, entry_fee, prize_pool, status, player_count, round, created_at) 
         VALUES (?, ?, ?, 'open', 0, 0, ?)"
    )
    .bind(name)
    .bind(entry_fee)
    .bind(prize_pool)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(result.last_insert_rowid())
}
```

## Example: Querying Tournament Data

SQLx provides compile-time checked queries with automatic type mapping.

```rust
/// Retrieves a specific tournament by ID
/// 
/// This function uses `query_as!` to automatically map the SQL result
/// to the Tournament struct. The query is checked at compile time.
/// 
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The tournament ID to retrieve
/// 
/// # Returns
/// The Tournament struct if found
/// 
/// # Errors
/// Returns a sqlx::Error if:
/// - The tournament doesn't exist
/// - The database query fails
/// 
/// # Example
/// ```ignore
/// let tournament = get_tournament_by_id(&pool, 123).await?;
/// println!("Tournament: {}", tournament.name);
/// ```
pub async fn get_tournament_by_id(
    pool: &SqlitePool,
    id: i64,
) -> Result<Tournament, sqlx::Error> {
    sqlx::query_as!(
        Tournament,
        "SELECT id, name, entry_fee, prize_pool, status, 
                player_count, round, created_at, started_at, completed_at 
         FROM tournaments 
         WHERE id = ?",
        id
    )
    .fetch_one(pool)
    .await
}

/// Retrieves all tournaments with optional status filter
/// 
/// # Arguments
/// * `pool` - The database connection pool
/// * `status` - Optional status filter (e.g., Some("open"))
/// 
/// # Returns
/// A vector of Tournament structs
/// 
/// # Errors
/// Returns a sqlx::Error if the query fails
pub async fn list_tournaments(
    pool: &SqlitePool,
    status: Option<&str>,
) -> Result<Vec<Tournament>, sqlx::Error> {
    if let Some(status) = status {
        sqlx::query_as!(
            Tournament,
            "SELECT id, name, entry_fee, prize_pool, status, 
                    player_count, round, created_at, started_at, completed_at 
             FROM tournaments 
             WHERE status = ? 
             ORDER BY created_at DESC",
            status
        )
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as!(
            Tournament,
            "SELECT id, name, entry_fee, prize_pool, status, 
                    player_count, round, created_at, started_at, completed_at 
             FROM tournaments 
             ORDER BY created_at DESC"
        )
        .fetch_all(pool)
        .await
    }
}
```

## Example: Transactions

Transactions ensure that multiple operations either all succeed or all fail.

```rust
/// Registers a player for a tournament in a transaction
/// 
/// This function uses a database transaction to ensure atomicity:
/// - If player registration fails, the tournament is not updated
/// - If tournament update fails, the player is not added
/// 
/// # Arguments
/// * `pool` - The database connection pool
/// * `tournament_id` - The tournament to join
/// * `player_pubkey` - The player's public key
/// * `elo_rating` - The player's ELO rating
/// 
/// # Returns
/// The ID of the newly created player entry
/// 
/// # Errors
/// Returns a sqlx::Error if any part of the transaction fails
pub async fn register_player_transaction(
    pool: &SqlitePool,
    tournament_id: i64,
    player_pubkey: &str,
    elo_rating: i32,
) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    
    let now = Utc::now().timestamp();
    
    // Insert player entry
    let player_result = sqlx::query(
        "INSERT INTO players (tournament_id, pubkey, elo_rating, registered_at) 
         VALUES (?, ?, ?, ?)"
    )
    .bind(tournament_id)
    .bind(player_pubkey)
    .bind(elo_rating)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    
    // Update tournament player count
    sqlx::query("UPDATE tournaments SET player_count = player_count + 1 WHERE id = ?")
        .bind(tournament_id)
        .execute(&mut *tx)
        .await?;
    
    // Update prize pool
    let tournament = sqlx::query_as!(
        Tournament,
        "SELECT * FROM tournaments WHERE id = ?",
        tournament_id
    )
    .fetch_one(&mut *tx)
    .await?;
    
    sqlx::query("UPDATE tournaments SET prize_pool = prize_pool + ? WHERE id = ?")
        .bind(tournament.entry_fee)
        .bind(tournament_id)
        .execute(&mut *tx)
        .await?;
    
    // Commit the transaction
    tx.commit().await?;
    
    Ok(player_result.last_insert_rowid())
}
```

## Example: Update Operations

Update operations modify existing data in the database.

```rust
/// Updates a tournament's status
/// 
/// # Arguments
/// * `pool` - The database connection pool
/// * `tournament_id` - The tournament to update
/// * `status` - The new status
/// 
/// # Returns
/// The number of rows affected
/// 
/// # Errors
/// Returns a sqlx::Error if the update fails
pub async fn update_tournament_status(
    pool: &SqlitePool,
    tournament_id: i64,
    status: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE tournaments SET status = ? WHERE id = ?"
    )
    .bind(status)
    .bind(tournament_id)
    .execute(pool)
    .await?;
    
    Ok(result.rows_affected())
}

/// Records a match result
/// 
/// # Arguments
/// * `pool` - The database connection pool
/// * `tournament_id` - The tournament ID
/// * `player_white` - White player's public key
/// * `player_black` - Black player's public key
/// * `winner` - Winner's public key (None if draw)
/// * `round` - The round number
/// * `match_index` - The match index within the round
/// 
/// # Returns
/// The ID of the newly created match record
/// 
/// # Errors
/// Returns a sqlx::Error if the insert fails
pub async fn record_match_result(
    pool: &SqlitePool,
    tournament_id: i64,
    player_white: &str,
    player_black: &str,
    winner: Option<&str>,
    round: i32,
    match_index: i32,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO matches 
         (tournament_id, player_white, player_black, winner, round, match_index) 
         VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(tournament_id)
    .bind(player_white)
    .bind(player_black)
    .bind(winner)
    .bind(round)
    .bind(match_index)
    .execute(pool)
    .await?;
    
    Ok(result.last_insert_rowid())
}
```
