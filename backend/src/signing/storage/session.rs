//! Session storage for the XFChess signing service.
//!
//! This module provides SQLite-backed storage for game sessions,
//! including session keypair management and user authentication.

use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Session entry containing keypair and wallet information.
#[derive(Clone)]
pub struct SessionEntry {
    /// The 64-byte session keypair (secret + public)
    pub keypair_bytes: [u8; 64],
    /// The wallet public key that owns this session
    pub wallet_pubkey: Pubkey,
    /// Whether the session is currently active (game in progress)
    pub active: bool,
}

impl SessionEntry {
    /// Extracts the Keypair from stored bytes.
    pub fn keypair(&self) -> Keypair {
        Keypair::try_from(self.keypair_bytes.as_slice()).expect("valid keypair bytes")
    }

    /// Gets the session's public key.
    pub fn session_pubkey(&self) -> Pubkey {
        self.keypair().pubkey()
    }
}

/// SQLite-backed session store that persists across server restarts.
#[derive(Clone)]
pub struct SessionStore {
    pool: SqlitePool,
}

impl SessionStore {
    /// Creates a new SessionStore with the provided database pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initializes the sessions table if it doesn't exist.
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                game_id   INTEGER PRIMARY KEY,
                keypair   BLOB    NOT NULL,
                wallet    TEXT    NOT NULL,
                active    INTEGER NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                email         TEXT PRIMARY KEY,
                password_hash TEXT NOT NULL,
                username      TEXT NOT NULL,
                wallet        TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Finds a user by email.
    pub async fn find_user(&self, email: &str) -> Option<(String, String, String, Option<String>)> {
        sqlx::query_as("SELECT email, password_hash, username, wallet FROM users WHERE email = ?")
            .bind(email)
            .fetch_one(&self.pool)
            .await
            .ok()
    }

    /// Finds a user by linked wallet public key.
    pub async fn find_user_by_wallet(&self, wallet: &str) -> Option<(String, String, String, Option<String>)> {
        sqlx::query_as("SELECT email, password_hash, username, wallet FROM users WHERE wallet = ?")
            .bind(wallet)
            .fetch_one(&self.pool)
            .await
            .ok()
    }

    /// Creates a new user.
    pub async fn create_user(&self, email: &str, password_hash: &str, username: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO users (email, password_hash, username) VALUES (?, ?, ?)")
            .bind(email)
            .bind(password_hash)
            .bind(username)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Links a wallet to an existing user.
    pub async fn link_wallet(&self, email: &str, wallet_pubkey: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET wallet = ? WHERE email = ?")
            .bind(wallet_pubkey)
            .bind(email)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Creates a new session for the given game and wallet.
    ///
    /// Generates a fresh session keypair and stores it in the database.
    ///
    /// # Arguments
    /// * `game_id` - The unique game identifier
    /// * `wallet_pubkey` - The wallet public key that owns this session
    ///
    /// # Returns
    /// The session's public key
    pub async fn create(&self, game_id: u64, wallet_pubkey: Pubkey) -> Pubkey {
        let kp = Keypair::new();
        let pubkey = kp.pubkey();
        let keypair_bytes = kp.to_bytes();
        let wallet_str = wallet_pubkey.to_string();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO sessions (game_id, keypair, wallet, active)
            VALUES (?1, ?2, ?3, 0)
            "#,
        )
        .bind(game_id as i64)
        .bind(&keypair_bytes[..])
        .bind(wallet_str)
        .execute(&self.pool)
        .await
        .expect("Failed to insert session");

        pubkey
    }

    /// Retrieves a session entry by game ID.
    pub async fn get(&self, game_id: u64) -> Option<SessionEntry> {
        let row: (Vec<u8>, String, i64) = sqlx::query_as(
            "SELECT keypair, wallet, active FROM sessions WHERE game_id = ?"
        )
        .bind(game_id as i64)
        .fetch_one(&self.pool)
        .await
        .ok()?;

        let mut keypair_bytes = [0u8; 64];
        keypair_bytes.copy_from_slice(&row.0);

        let wallet_pubkey = Pubkey::from_str(&row.1).ok()?;
        let active = row.2 != 0;

        Some(SessionEntry {
            keypair_bytes,
            wallet_pubkey,
            active,
        })
    }

    /// Marks a session as active (game started).
    pub async fn activate(&self, game_id: u64) {
        sqlx::query("UPDATE sessions SET active = 1 WHERE game_id = ?")
            .bind(game_id as i64)
            .execute(&self.pool)
            .await
            .ok();
    }

    /// Checks if a session is currently active.
    pub async fn is_active(&self, game_id: u64) -> bool {
        let (active,): (i64,) = sqlx::query_as("SELECT active FROM sessions WHERE game_id = ?")
            .bind(game_id as i64)
            .fetch_one(&self.pool)
            .await
            .unwrap_or((0,));
        active != 0
    }

    /// Counts active sessions (currently running games).
    pub async fn count_active(&self) -> u64 {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE active = 1")
            .fetch_one(&self.pool)
            .await
            .unwrap_or((0,));
        count as u64
    }

    /// Counts total unique players (by wallet pubkey).
    pub async fn count_unique_players(&self) -> u64 {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(DISTINCT wallet) FROM sessions")
            .fetch_one(&self.pool)
            .await
            .unwrap_or((0,));
        count as u64
    }

    /// Counts total sessions ever created.
    pub async fn count_total_sessions(&self) -> u64 {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions")
            .fetch_one(&self.pool)
            .await
            .unwrap_or((0,));
        count as u64
    }
}
