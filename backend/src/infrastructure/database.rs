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
        .max_connections(16)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .idle_timeout(std::time::Duration::from_secs(300))
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
    // Helper to run a semicolon-separated SQL script on a pool
    async fn run_script(pool: &sqlx::SqlitePool, script: &str, name: &str) -> Result<(), sqlx::Error> {
        // Strip comments to avoid breaking on semicolons inside comments
        let mut clean_script = String::new();
        for line in script.lines() {
            if let Some(comment_start) = line.find("--") {
                clean_script.push_str(&line[..comment_start]);
            } else {
                clean_script.push_str(line);
            }
            clean_script.push(' '); // Replace newline with space
        }

        for statement in clean_script.split(';') {
            let statement = statement.trim();
            if statement.is_empty() {
                continue;
            }
            if let Err(e) = sqlx::query(statement).execute(pool).await {
                // For ALTER TABLE, we ignore "duplicate column" errors
                let err_msg = e.to_string().to_lowercase();
                if err_msg.contains("duplicate column") || err_msg.contains("already exists") {
                    continue;
                }
                tracing::error!("Migration {} failed on statement: {}: {}", name, statement, e);
                return Err(e);
            }
        }
        Ok(())
    }

    // ── Migration 001: initial schema ─────────────────────────────────────────
    let migration_001 = include_str!("../../migrations/001_initial.sql");
    run_script(&pools.session_pool, migration_001, "001 (session)").await?;
    run_script(&pools.vault_pool, migration_001, "001 (vault)").await?;

    // ── Migration 002: GDPR KYC tables ────────────────────────────────────────
    let migration_002 = include_str!("../../migrations/002_kyc_gdpr.sql");
    run_script(&pools.vault_pool, migration_002, "002 (vault)").await?;
    run_script(&pools.session_pool, migration_002, "002 (session)").await?;

    // ── Migration 003: wallet-first auth ──────────────────────────────────────
    let migration_003 = include_str!("../../migrations/003_wallet_first_auth.sql");
    run_script(&pools.session_pool, migration_003, "003").await?;

    // ── Migration 004: performance indexes ────────────────────────────────────
    let migration_004 = include_str!("../../migrations/004_indexes_and_wal.sql");
    run_script(&pools.vault_pool, migration_004, "004 (vault)").await?;
    run_script(&pools.session_pool, migration_004, "004 (session)").await?;

    // ── Migration 005: disputes table ─────────────────────────────────────────
    let migration_005 = include_str!("../../migrations/005_disputes.sql");
    run_script(&pools.session_pool, migration_005, "005").await?;

    // ── Migration 006: game history + move counter ────────────────────────────
    let migration_006 = include_str!("../../migrations/006_game_history.sql");
    run_script(&pools.session_pool, migration_006, "006").await?;

    // ── Migration 007: add password_hash ──────────────────────────────────────
    let migration_007 = include_str!("../../migrations/007_add_password_hash.sql");
    run_script(&pools.session_pool, migration_007, "007").await?;

    // ── Migration 008: anti-cheat tables ──────────────────────────────────────
    let migration_008 = include_str!("../../migrations/008_anticheat.sql");
    run_script(&pools.session_pool, migration_008, "008").await?;

    // ── Migration 009: external ELO ───────────────────────────────────────────
    let migration_009 = include_str!("../../migrations/009_external_elo.sql");
    run_script(&pools.session_pool, migration_009, "009").await?;

    // ── Migration 010: CACF compliance ────────────────────────────────────────
    let migration_010 = include_str!("../../migrations/010_cacf_compliance.sql");
    run_script(&pools.session_pool, migration_010, "010").await?;

    // ── Migration 011: friends/social ─────────────────────────────────────────
    let migration_011 = include_str!("../../migrations/011_friends.sql");
    run_script(&pools.session_pool, migration_011, "011").await?;

    // ── Migration 012: performance indexes ────────────────────────────────────
    let migration_012 = include_str!("../../migrations/012_perf_indexes.sql");
    run_script(&pools.session_pool, migration_012, "012").await?;

    // ── Migration 018: puzzle pool + bounties ──────────────────────────────────
    // (013–017 are created idempotently by SessionStore::init; 018 is applied
    // here because the puzzle tables have no init hook of their own.)
    let migration_018 = include_str!("../../migrations/018_puzzles.sql");
    run_script(&pools.session_pool, migration_018, "018").await?;

    // ── Migration 019: durable job queue (WS-A) ───────────────────────────────
    let migration_019 = include_str!("../../migrations/019_job_queue.sql");
    run_script(&pools.session_pool, migration_019, "019").await?;

    info!("[Database] All migrations completed successfully");
    Ok(())
}
