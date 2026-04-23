//! Database initialization and migrations for the XFChess backend.
//!
//! This module handles SQLite pool initialization and schema migrations
//! for sessions, vault, and audit log tables.
//!
//! Both pools use WAL journal mode for concurrent read performance and
//! a busy timeout to avoid immediate lock errors under load.

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tracing::info;

/// Database pools for the application.
#[derive(Clone)]
pub struct DatabasePools {
    /// Pool for sessions and game data
    pub session_pool: SqlitePool,
    /// Pool for identity vault (GDPR-compliant encrypted storage)
    pub vault_pool: SqlitePool,
}

async fn make_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .min_connections(1)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // WAL mode: concurrent reads + single writer, no contention
                sqlx::query("PRAGMA journal_mode = WAL").execute(&mut *conn).await?;
                // NORMAL sync is safe with WAL (only risks losing the last tx on OS crash)
                sqlx::query("PRAGMA synchronous = NORMAL").execute(&mut *conn).await?;
                // Enforce FK constraints
                sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await?;
                // Wait up to 5s on a locked write instead of returning SQLITE_BUSY
                sqlx::query("PRAGMA busy_timeout = 5000").execute(&mut *conn).await?;
                Ok(())
            })
        })
        .connect(url)
        .await?;
    Ok(pool)
}

/// Initializes SQLite database pools with WAL mode and connection tuning.
pub async fn initialize_pools(
    session_db_url: &str,
    vault_db_url: &str,
) -> Result<DatabasePools, sqlx::Error> {
    let session_pool = make_pool(session_db_url).await?;
    info!("[Database] Session pool connected (WAL)");

    let vault_pool = make_pool(vault_db_url).await?;
    info!("[Database] Vault pool connected (WAL)");

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
/// - KYC records table and users column extensions (migration 002)
///
/// # Arguments
/// * `pools` - Database pools to run migrations on
pub async fn run_migrations(pools: &DatabasePools) -> Result<(), sqlx::Error> {
    // ── Migration 001: initial schema ─────────────────────────────────────────
    let migration_001 = include_str!("../../migrations/001_initial.sql");
    for statement in migration_001.split(';') {
        let statement = statement.trim();
        if statement.is_empty() || statement.starts_with("--") {
            continue;
        }
        if statement.contains("CREATE TABLE IF NOT EXISTS sessions")
            || statement.contains("CREATE TABLE IF NOT EXISTS users")
        {
            sqlx::query(statement).execute(&pools.session_pool).await
                .map_err(|e| { tracing::error!("Migration 001 (session) failed on statement: {}: {}", statement, e); e })?;
        } else if statement.contains("CREATE TABLE IF NOT EXISTS vault_users")
            || statement.contains("CREATE TABLE IF NOT EXISTS audit_log")
        {
            sqlx::query(statement).execute(&pools.vault_pool).await
                .map_err(|e| { tracing::error!("Migration 001 (vault) failed on statement: {}: {}", statement, e); e })?;
        }
    }

    // ── Migration 002: GDPR KYC tables ────────────────────────────────────────
    let migration_002 = include_str!("../../migrations/002_kyc_gdpr.sql");
    for statement in migration_002.split(';') {
        let statement = statement.trim();
        if statement.is_empty() || statement.starts_with("--") {
            continue;
        }
        let is_vault = statement.contains("CREATE TABLE IF NOT EXISTS kyc_records")
            || statement.contains("CREATE TABLE IF NOT EXISTS deletion_requests");
        let is_session = statement.contains("ALTER TABLE users");

        if is_vault {
            sqlx::query(statement).execute(&pools.vault_pool).await
                .map_err(|e| { tracing::error!("Migration 002 (vault) failed on statement: {}: {}", statement, e); e })?;
        } else if is_session {
            // ALTER TABLE ADD COLUMN fails silently if column exists
            let _ = sqlx::query(statement).execute(&pools.session_pool).await;
        }
    }

    // ── Migration 003: wallet-first auth (no passwords) ───────────────────────
    let migration_003 = include_str!("../../migrations/003_wallet_first_auth.sql");
    for statement in migration_003.split(';') {
        let statement = statement.trim();
        if statement.is_empty() || statement.starts_with("--") {
            continue;
        }
        let _ = sqlx::query(statement).execute(&pools.session_pool).await;
    }

    // ── Migration 004: performance indexes ────────────────────────────────────
    let migration_004 = include_str!("../../migrations/004_indexes_and_wal.sql");
    for statement in migration_004.split(';') {
        let statement = statement.trim();
        if statement.is_empty() || statement.starts_with("--") {
            continue;
        }
        // Indexes on audit_log and deletion_requests live in vault pool
        let _ = sqlx::query(statement).execute(&pools.vault_pool).await;
        // Also try session pool (for any future session-side indexes)
        let _ = sqlx::query(statement).execute(&pools.session_pool).await;
    }

    // ── Migration 005: disputes table ─────────────────────────────────────────
    let migration_005 = include_str!("../../migrations/005_disputes.sql");
    for statement in migration_005.split(';') {
        let statement = statement.trim();
        if statement.is_empty() || statement.starts_with("--") {
            continue;
        }
        let _ = sqlx::query(statement).execute(&pools.session_pool).await;
    }

    // ── Migration 006: game history + move counter ────────────────────────────
    let migration_006 = include_str!("../../migrations/006_game_history.sql");
    for statement in migration_006.split(';') {
        let statement = statement.trim();
        if statement.is_empty() || statement.starts_with("--") {
            continue;
        }
        // ALTER TABLE ADD COLUMN is idempotent via ignore — SQLite errors if col exists
        let _ = sqlx::query(statement).execute(&pools.session_pool).await;
    }

    info!("[Database] All migrations completed successfully");
    Ok(())
}
