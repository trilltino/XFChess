//! Database initialization and migrations for the XFChess backend.
//!
//! This module handles SQLite pool initialization and schema migrations
//! for sessions, vault, and audit log tables.

use sqlx::SqlitePool;
use tracing::info;

/// Database pools for the application.
#[derive(Clone)]
pub struct DatabasePools {
    /// Pool for sessions and game data
    pub session_pool: SqlitePool,
    /// Pool for identity vault (GDPR-compliant encrypted storage)
    pub vault_pool: SqlitePool,
}

/// Initializes SQLite database pools.
///
/// # Arguments
/// * `session_db_url` - SQLite connection URL for sessions database
/// * `vault_db_url` - SQLite connection URL for vault database
///
/// # Returns
/// A `DatabasePools` struct containing both initialized pools
pub async fn initialize_pools(
    session_db_url: &str,
    vault_db_url: &str,
) -> Result<DatabasePools, sqlx::Error> {
    let session_pool = SqlitePool::connect(session_db_url).await?;
    info!("[Database] Session pool connected");

    let vault_pool = SqlitePool::connect(vault_db_url).await?;
    info!("[Database] Vault pool connected");

    Ok(DatabasePools {
        session_pool,
        vault_pool,
    })
}

/// Runs database migrations for all tables.
///
/// This function creates all necessary tables if they don't exist:
/// - Sessions table (for game session keys)
/// - Users table (for authentication)
/// - Vault table (for encrypted identity data)
/// - Audit log table (for GDPR compliance)
///
/// # Arguments
/// * `pools` - Database pools to run migrations on
pub async fn run_migrations(pools: &DatabasePools) -> Result<(), sqlx::Error> {
    // Run initial migration from SQL file
    let migration_sql = include_str!("../../migrations/001_initial.sql");
    
    // Execute migrations on session pool
    for statement in migration_sql.split(';') {
        let statement = statement.trim();
        if statement.is_empty() {
            continue;
        }
        
        // Determine which pool to use based on the table
        if statement.contains("CREATE TABLE IF NOT EXISTS sessions") 
            || statement.contains("CREATE TABLE IF NOT EXISTS users") {
            sqlx::query(statement).execute(&pools.session_pool).await?;
        } else if statement.contains("CREATE TABLE IF NOT EXISTS vault_users")
            || statement.contains("CREATE TABLE IF NOT EXISTS audit_log") {
            sqlx::query(statement).execute(&pools.vault_pool).await?;
        }
    }

    info!("[Database] All migrations completed successfully");
    Ok(())
}
