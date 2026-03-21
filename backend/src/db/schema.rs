use sqlx::{SqlitePool, migrate::MigrateDatabase, Sqlite};
use anyhow::Result;
use std::str::FromStr;

/// Initialize the SQLite database with required tables
pub async fn init_db(database_url: &str) -> Result<SqlitePool> {
    // Create database if it doesn't exist
    if !Sqlite::database_exists(database_url).await? {
        Sqlite::create_database(database_url).await?;
        println!("Created database: {}", database_url);
    }

    // Connect to database with connection pool configuration
    let pool = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(30))
    ).await?;

    // Run migrations
    create_tables(&pool).await?;

    println!("Database initialized successfully with WAL mode and connection pooling");
    Ok(pool)
}

/// Create the games and moves tables
async fn create_tables(pool: &SqlitePool) -> Result<()> {
    // Games table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS games (
            id TEXT PRIMARY KEY,
            player_white TEXT NOT NULL,
            player_black TEXT NOT NULL,
            stake_amount REAL DEFAULT 0.0,
            start_time INTEGER NOT NULL,
            end_time INTEGER,
            winner TEXT,
            final_fen TEXT,
            status TEXT DEFAULT 'playing',
            created_at INTEGER DEFAULT (strftime('%s', 'now'))
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Moves table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS moves (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            game_id TEXT NOT NULL,
            move_number INTEGER NOT NULL,
            move_uci TEXT NOT NULL,
            move_san TEXT,
            fen_before TEXT,
            fen_after TEXT,
            player TEXT NOT NULL,
            timestamp INTEGER DEFAULT (strftime('%s', 'now')),
            FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create indexes for performance
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_games_start_time ON games(start_time)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_games_players ON games(player_white, player_black)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_moves_game_id ON moves(game_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_moves_timestamp ON moves(timestamp)")
        .execute(pool)
        .await?;

    println!("Tables and indexes created successfully");
    Ok(())
}

/// Clean up old games (optional maintenance)
pub async fn cleanup_old_games(pool: &SqlitePool, days_old: u32) -> Result<u64> {
    let cutoff_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() - (days_old as u64 * 24 * 60 * 60);

    let result = sqlx::query(
        "DELETE FROM games WHERE start_time < ? AND status IN ('completed', 'aborted')"
    )
    .bind(cutoff_time as i64)
    .execute(pool)
    .await?;

    let deleted_count = result.rows_affected();
    if deleted_count > 0 {
        println!("Cleaned up {} old games", deleted_count);
    }

    Ok(deleted_count)
}
