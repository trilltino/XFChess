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
        Keypair::try_from(self.keypair_bytes.as_slice())
            .unwrap_or_else(|e| panic!("invalid keypair bytes: {}", e))
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

    /// Returns a clone of the underlying pool for use in repositories.
    pub fn pool(&self) -> SqlitePool {
        self.pool.clone()
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
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Wallet-first user table — no password_hash
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users_v2 (
                wallet      TEXT PRIMARY KEY,
                username    TEXT NOT NULL,
                email       TEXT UNIQUE,
                password_hash TEXT,
                kyc_status  TEXT NOT NULL DEFAULT 'none',
                created_at  INTEGER NOT NULL DEFAULT 0,
                deleted_at  INTEGER
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_users_v2_username ON users_v2 (LOWER(username));"
        )
        .execute(&self.pool)
        .await?;

        // Client-side anti-cheat telemetry (blur + think-time reporting) —
        // mirrors migrations/013_move_telemetry.sql and 014_think_time.sql for
        // deployments that don't run sqlx migrations.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS move_telemetry (
                game_id     TEXT    NOT NULL,
                move_number INTEGER NOT NULL,
                color       TEXT    NOT NULL,
                blurred     INTEGER NOT NULL DEFAULT 0,
                think_ms    INTEGER,
                reported_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                PRIMARY KEY (game_id, move_number)
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Defensive ALTER for DBs created before migration 014. SQLite has no
        // ADD COLUMN IF NOT EXISTS; a duplicate-column error here is expected
        // and harmless.
        let _ = sqlx::query("ALTER TABLE move_telemetry ADD COLUMN think_ms INTEGER")
            .execute(&self.pool)
            .await;

        // Per-game broadcast delay (migration 015) — same guarded-ALTER pattern.
        let _ = sqlx::query(
            "ALTER TABLE games ADD COLUMN broadcast_delay_secs INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;

        // Reconcile the migration-006 `games`/`moves` schema with the columns
        // the repository actually reads/writes. Migration 006 created the
        // minimal tables; `pgn_text`, `move_san`, and `fen_before` live only in
        // the alternate `db::schema` init path that the signing-server does not
        // run. Without these, SAN/PGN persistence and `SELECT *` of GameRecord
        // fail (silently, on fire-and-forget paths). Guarded ALTERs are
        // idempotent across restarts.
        let _ = sqlx::query("ALTER TABLE games ADD COLUMN pgn_text TEXT")
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("ALTER TABLE moves ADD COLUMN move_san TEXT")
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("ALTER TABLE moves ADD COLUMN fen_before TEXT")
            .execute(&self.pool)
            .await;

        // Account-linkage / Sybil signals (migration 016).
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS account_linkage (
                wallet        TEXT    PRIMARY KEY,
                funder        TEXT,
                device_hash   TEXT,
                ip_count      INTEGER NOT NULL DEFAULT 0,
                flagged       INTEGER NOT NULL DEFAULT 0,
                hard_blocked  INTEGER NOT NULL DEFAULT 0,
                first_seen    INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                last_seen     INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            "#,
        )
        .execute(&self.pool)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_linkage_funder ON account_linkage(funder)")
            .execute(&self.pool)
            .await
            .ok();
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_linkage_device ON account_linkage(device_hash)",
        )
        .execute(&self.pool)
        .await
        .ok();

        // JWT revocation cut-offs (migration 017). A logout records the current
        // time for a subject; any token issued at or before `valid_after` is then
        // rejected, giving us a kill switch for the otherwise non-revocable JWTs.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS jwt_revocations (
                subject     TEXT    PRIMARY KEY,
                valid_after INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Revokes all JWTs for `subject` issued at or before `valid_after` (Unix
    /// seconds). Used by logout; safe to call repeatedly (last write wins).
    pub async fn revoke_tokens_before(
        &self,
        subject: &str,
        valid_after: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO jwt_revocations (subject, valid_after) VALUES (?, ?)
             ON CONFLICT(subject) DO UPDATE SET valid_after = MAX(valid_after, excluded.valid_after)",
        )
        .bind(subject)
        .bind(valid_after)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns `true` if a token for `subject` issued at `iat` has been revoked.
    /// Tokens without an `iat` (legacy, `iat == 0`) are treated as revoked once a
    /// cut-off exists for the subject, forcing a fresh login.
    pub async fn token_is_revoked(&self, subject: &str, iat: i64) -> bool {
        let cutoff: Option<(i64,)> =
            sqlx::query_as("SELECT valid_after FROM jwt_revocations WHERE subject = ?")
                .bind(subject)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();
        match cutoff {
            // Strict `<` so a token re-issued in the same second as the logout
            // (a normal logout-then-login) is not itself revoked.
            Some((valid_after,)) => iat < valid_after,
            None => false,
        }
    }

    /// Finds a user by wallet pubkey. Returns (wallet, username, email, kyc_status, password_hash).
    pub async fn find_user_by_wallet(
        &self,
        wallet: &str,
    ) -> Option<(String, String, Option<String>, String, Option<String>)> {
        sqlx::query_as(
            "SELECT wallet, username, email, kyc_status, password_hash FROM users_v2 WHERE wallet = ? AND deleted_at IS NULL",
        )
        .bind(wallet)
        .fetch_one(&self.pool)
        .await
        .ok()
    }

    /// Finds a user by email.
    pub async fn find_user_by_email(
        &self,
        email: &str,
    ) -> Option<(String, String, Option<String>, String, Option<String>)> {
        sqlx::query_as(
            "SELECT wallet, username, email, kyc_status, password_hash FROM users_v2 WHERE email = ? AND deleted_at IS NULL",
        )
        .bind(email)
        .fetch_one(&self.pool)
        .await
        .ok()
    }

    /// Creates a new user with email and password.
    pub async fn register_with_email(
        &self,
        email: &str,
        username: &str,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO users_v2 (wallet, username, email, password_hash, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("") // Wallet is empty until linked
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Links a wallet to an existing email-based account.
    pub async fn link_wallet(&self, email: &str, wallet: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET wallet = ? WHERE email = ?")
            .bind(wallet)
            .bind(email)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Sets the email on an existing wallet-first account.
    pub async fn set_email(&self, wallet: &str, email: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET email = ? WHERE wallet = ?")
            .bind(email)
            .bind(wallet)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Creates a new wallet-first user.
    pub async fn create_wallet_user(
        &self,
        wallet: &str,
        username: &str,
        email: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO users_v2 (wallet, username, email, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(wallet)
        .bind(username)
        .bind(email)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Overwrites the username for a wallet (used when syncing from on-chain profile).
    pub async fn update_username(&self, wallet: &str, username: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET username = ? WHERE wallet = ?")
            .bind(username)
            .bind(wallet)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Updates kyc_status for a wallet.
    pub async fn set_kyc_status(&self, wallet: &str, status: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET kyc_status = ? WHERE wallet = ?")
            .bind(status)
            .bind(wallet)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Returns `Some(unix_ts)` if this account already received a
    /// backend-sponsored on-chain profile creation — guards against repeat
    /// sponsorship of the same wallet.
    pub async fn profile_sponsored_at(&self, wallet: &str) -> Option<i64> {
        let row: Option<(Option<i64>,)> =
            sqlx::query_as("SELECT profile_sponsored_at FROM users_v2 WHERE wallet = ?")
                .bind(wallet)
                .fetch_one(&self.pool)
                .await
                .ok();
        row.and_then(|(v,)| v)
    }

    /// Marks this account as having received its one backend-sponsored
    /// profile creation.
    pub async fn mark_profile_sponsored(&self, wallet: &str, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET profile_sponsored_at = ? WHERE wallet = ?")
            .bind(now)
            .bind(wallet)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Records a casual (off-chain) game result — bot or local-P2P play,
    /// with no on-chain effect. `account_id` is a wallet pubkey or the
    /// `"email:<addr>"` JWT subject.
    pub async fn record_casual_game(
        &self,
        account_id: &str,
        opponent_type: &str,
        result: &str,
        pgn: Option<&str>,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO casual_games (account_id, opponent_type, result, pgn, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(account_id)
        .bind(opponent_type)
        .bind(result)
        .bind(pgn)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns true if the given username is already taken (case-insensitive).
    pub async fn username_taken(&self, username: &str) -> bool {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM users_v2 WHERE LOWER(username) = LOWER(?) AND deleted_at IS NULL",
        )
        .bind(username)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0,));
        count > 0
    }

    /// GDPR erasure: soft-deletes user and nulls PII fields.
    pub async fn erase_user(&self, wallet: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "UPDATE users_v2 SET username = '[erased]', email = NULL, deleted_at = ? WHERE wallet = ?",
        )
        .bind(now)
        .bind(wallet)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Creates a new session for the given game and wallet, or returns the
    /// existing session pubkey if one already exists for this game_id.
    ///
    /// Using get-or-create semantics ensures that the joiner calling
    /// create_session with the same game_id gets back the same session pubkey
    /// that was stored in game.fee_payer during create_game, preventing
    /// FeePayerMismatch errors in join_game.
    ///
    /// # Arguments
    /// * `game_id` - The unique game identifier
    /// * `wallet_pubkey` - The wallet public key that owns this session
    ///
    /// # Returns
    /// The session's public key
    pub async fn create(&self, game_id: u64, wallet_pubkey: Pubkey) -> anyhow::Result<Pubkey> {
        // Return the existing session pubkey if one already exists for this game.
        if let Some(existing) = self.get(game_id).await {
            return Ok(existing.session_pubkey());
        }

        let kp = Keypair::new();
        let pubkey = kp.pubkey();
        let keypair_bytes = kp.to_bytes();
        let wallet_str = wallet_pubkey.to_string();

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO sessions (game_id, keypair, wallet, active)
            VALUES (?1, ?2, ?3, 0)
            "#,
        )
        .bind(game_id as i64)
        .bind(&keypair_bytes[..])
        .bind(wallet_str)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert session: {}", e);
            anyhow::anyhow!("Failed to insert session: {}", e)
        })?;

        Ok(pubkey)
    }

    /// Retrieves a session entry by game ID.
    pub async fn get(&self, game_id: u64) -> Option<SessionEntry> {
        let row: (Vec<u8>, String, i64) =
            sqlx::query_as("SELECT keypair, wallet, active FROM sessions WHERE game_id = ?")
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

    /// Marks a session as inactive (game settled or abandoned).
    pub async fn deactivate(&self, game_id: u64) {
        sqlx::query("UPDATE sessions SET active = 0 WHERE game_id = ?")
            .bind(game_id as i64)
            .execute(&self.pool)
            .await
            .ok();
    }

    /// Lists the game IDs of all currently active sessions.
    pub async fn list_active_game_ids(&self) -> Vec<u64> {
        sqlx::query_as::<_, (i64,)>("SELECT game_id FROM sessions WHERE active = 1")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(id,)| id as u64)
            .collect()
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

    /// Atomically increments the move counter for a game and returns the new value.
    /// Used to assign sequential move numbers when persisting moves to the DB.
    pub async fn increment_move_count(&self, game_id: u64) -> i32 {
        let result: Result<(i64,), _> = sqlx::query_as(
            "UPDATE sessions SET move_count = move_count + 1 WHERE game_id = ? RETURNING move_count",
        )
        .bind(game_id as i64)
        .fetch_one(&self.pool)
        .await;
        result.map(|(n,)| n as i32).unwrap_or(1)
    }

    /// Lists all players in the system.
    pub async fn list_players(
        &self,
        limit: i32,
    ) -> Result<Vec<(String, String, String)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT wallet, username, kyc_status FROM users_v2 WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
}
