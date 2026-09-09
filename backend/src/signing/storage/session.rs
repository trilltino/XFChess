use crate::signing::identity::IdentityVault;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use sqlx::SqlitePool;
use std::str::FromStr;

#[derive(Clone)]
pub struct SessionEntry {
    pub keypair_bytes: [u8; 64],
    pub wallet_pubkey: Pubkey,
    pub active: bool,
    pub is_global: bool,
}

impl SessionEntry {
    pub fn keypair(&self) -> Keypair {
        Keypair::try_from(self.keypair_bytes.as_slice())
            .unwrap_or_else(|e| panic!("invalid keypair bytes: {}", e))
    }

    pub fn session_pubkey(&self) -> Pubkey {
        self.keypair().pubkey()
    }
}

#[derive(Clone)]
pub struct SessionStore {
    pool: SqlitePool,
    vault: IdentityVault,
}

impl SessionStore {
    pub fn new(pool: SqlitePool, vault: IdentityVault) -> Self {
        Self { pool, vault }
    }

    pub fn pool(&self) -> SqlitePool {
        self.pool.clone()
    }

    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                game_id   INTEGER PRIMARY KEY,
                keypair   BLOB    NOT NULL,
                wallet    TEXT    NOT NULL,
                active    INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Global-session flow marker (migration 026) — guarded ALTER, same
        // pattern as the other defensive ALTERs below. See that migration's
        // comment for why move-recording needs to tell the two flows apart.
        let _ = sqlx::query("ALTER TABLE sessions ADD COLUMN is_global INTEGER NOT NULL DEFAULT 0")
            .execute(&self.pool)
            .await;
        let _ =
            sqlx::query("ALTER TABLE sessions ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0")
                .execute(&self.pool)
                .await;

        // Per-wallet activation record (migration 025) — mirrors migrations/
        // for deployments that don't run sqlx migrations. See that file's
        // comment for why this can't just reuse `sessions.active`.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_wallet_activations (
                game_id INTEGER NOT NULL,
                wallet  TEXT    NOT NULL,
                sig     TEXT    NOT NULL,
                PRIMARY KEY (game_id, wallet)
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

    pub async fn link_wallet(&self, email: &str, wallet: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET wallet = ? WHERE email = ?")
            .bind(wallet)
            .bind(email)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_email(&self, wallet: &str, email: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET email = ? WHERE wallet = ?")
            .bind(email)
            .bind(wallet)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

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

    // ── Social identities (Privy Google/email → wallet) ────────────────────────

    pub async fn find_wallet_by_social(&self, provider: &str, subject: &str) -> Option<String> {
        sqlx::query_as::<_, (String,)>(
            "SELECT wallet FROM social_identities WHERE provider = ? AND subject = ?",
        )
        .bind(provider)
        .bind(subject)
        .fetch_one(&self.pool)
        .await
        .ok()
        .map(|(w,)| w)
    }

    pub async fn find_wallet_by_social_email(&self, provider: &str, email: &str) -> Option<String> {
        sqlx::query_as::<_, (String,)>(
            "SELECT wallet FROM social_identities WHERE provider = ? AND LOWER(email) = LOWER(?)",
        )
        .bind(provider)
        .bind(email)
        .fetch_one(&self.pool)
        .await
        .ok()
        .map(|(w,)| w)
    }

    pub async fn upsert_social_identity(
        &self,
        provider: &str,
        subject: &str,
        wallet: &str,
        login_method: &str,
        email: Option<&str>,
        embedded: bool,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO social_identities \
               (provider, subject, wallet, login_method, email, embedded, created_at, last_login_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(provider, subject) DO UPDATE SET \
               last_login_at = excluded.last_login_at, \
               login_method  = excluded.login_method",
        )
        .bind(provider)
        .bind(subject)
        .bind(wallet)
        .bind(login_method)
        .bind(email)
        .bind(if embedded { 1 } else { 0 })
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn social_login_methods(&self, wallet: &str) -> Vec<String> {
        sqlx::query_as::<_, (String,)>(
            "SELECT DISTINCT login_method FROM social_identities WHERE wallet = ?",
        )
        .bind(wallet)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(|(m,)| m).collect())
        .unwrap_or_default()
    }

    pub async fn wallet_is_embedded(&self, wallet: &str) -> bool {
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM social_identities WHERE wallet = ? AND embedded = 1",
        )
        .bind(wallet)
        .fetch_one(&self.pool)
        .await
        .map(|(n,)| n > 0)
        .unwrap_or(false)
    }

    pub async fn count_social_identities_since(&self, since: i64) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM social_identities WHERE created_at >= ?")
            .bind(since)
            .fetch_one(&self.pool)
            .await
            .map(|(n,)| n)
            .unwrap_or(0)
    }

    pub async fn update_username(&self, wallet: &str, username: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET username = ? WHERE wallet = ?")
            .bind(username)
            .bind(wallet)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_kyc_status(&self, wallet: &str, status: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET kyc_status = ? WHERE wallet = ?")
            .bind(status)
            .bind(wallet)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn profile_sponsored_at(&self, wallet: &str) -> Option<i64> {
        let row: Option<(Option<i64>,)> =
            sqlx::query_as("SELECT profile_sponsored_at FROM users_v2 WHERE wallet = ?")
                .bind(wallet)
                .fetch_one(&self.pool)
                .await
                .ok();
        row.and_then(|(v,)| v)
    }

    pub async fn mark_profile_sponsored(&self, wallet: &str, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users_v2 SET profile_sponsored_at = ? WHERE wallet = ?")
            .bind(now)
            .bind(wallet)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

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

    pub async fn create(&self, game_id: u64, wallet_pubkey: Pubkey) -> anyhow::Result<Pubkey> {
        // Return the existing session pubkey if one already exists for this game.
        if let Some(existing) = self.get(game_id).await {
            return Ok(existing.session_pubkey());
        }

        let kp = Keypair::new();
        let pubkey = kp.pubkey();
        let keypair_bytes = kp.to_bytes();
        let wallet_str = wallet_pubkey.to_string();
        let encrypted = self
            .vault
            .encrypt_bytes(&keypair_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to encrypt session keypair: {e}"))?;

        let insert = sqlx::query(
            r#"
            INSERT OR IGNORE INTO sessions (game_id, keypair, wallet, active, created_at)
            VALUES (?1, ?2, ?3, 0, strftime('%s', 'now'))
            "#,
        )
        .bind(game_id as i64)
        .bind(&encrypted[..])
        .bind(&wallet_str)
        .execute(&self.pool)
        .await;
        if let Err(e) = insert {
            if !e.to_string().contains("no column named created_at") {
                return Err(anyhow::anyhow!("Failed to insert session: {e}"));
            }
            sqlx::query(
                "INSERT OR IGNORE INTO sessions (game_id, keypair, wallet, active)
                 VALUES (?1, ?2, ?3, 0)",
            )
            .bind(game_id as i64)
            .bind(&encrypted[..])
            .bind(wallet_str)
            .execute(&self.pool)
            .await
            .map_err(|fallback| anyhow::anyhow!("Failed to insert session: {fallback}"))?;
        }

        Ok(pubkey)
    }

    pub async fn count_pending_for_wallet(&self, wallet: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sessions s
                         WHERE s.wallet = ? AND s.active = 0
                             AND s.created_at >= strftime('%s', 'now') - 1800
                             AND NOT EXISTS (
                                     SELECT 1 FROM session_wallet_activations a
                                     WHERE a.game_id = s.game_id
                             )",
        )
        .bind(wallet)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0)
    }

    pub async fn abandon_unactivated(&self, game_id: u64, wallet: &str) -> bool {
        sqlx::query(
            "DELETE FROM sessions
             WHERE game_id = ? AND wallet = ? AND active = 0
               AND NOT EXISTS (
                   SELECT 1 FROM session_wallet_activations a
                   WHERE a.game_id = sessions.game_id
               )",
        )
        .bind(game_id as i64)
        .bind(wallet)
        .execute(&self.pool)
        .await
        .map(|result| result.rows_affected() == 1)
        .unwrap_or(false)
    }

    pub async fn create_with_keypair(
        &self,
        game_id: u64,
        wallet_pubkey: Pubkey,
        keypair_bytes: [u8; 64],
    ) -> anyhow::Result<()> {
        let wallet_str = wallet_pubkey.to_string();

        // Guard against silently repointing an in-use session's recorded
        // owner: `game_id` is client-generated, so a collision with an
        // existing row should be vanishingly rare — but the UPSERT below
        // previously overwrote `wallet` unconditionally on ANY conflict,
        // decoupling a session's recorded owner from whoever actually holds
        // it (see the identity audit: this was a real invariant-violation
        // gap, not just theoretical). Reject instead of silently
        // reassigning when an ACTIVE row already exists under a DIFFERENT
        // wallet; a re-call for the SAME wallet (retry, idempotent re-track)
        // still proceeds exactly as before.
        if let Some((existing_wallet, active)) = sqlx::query_as::<_, (String, i64)>(
            "SELECT wallet, active FROM sessions WHERE game_id = ?",
        )
        .bind(game_id as i64)
        .fetch_optional(&self.pool)
        .await?
        {
            if active != 0 && existing_wallet != wallet_str {
                anyhow::bail!(
                    "session for game {game_id} is already active under a different wallet"
                );
            }
        }

        let encrypted = self
            .vault
            .encrypt_bytes(&keypair_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to encrypt session keypair: {e}"))?;
        sqlx::query(
            r#"
            INSERT INTO sessions (game_id, keypair, wallet, active, is_global, created_at)
            VALUES (?1, ?2, ?3, 1, 1, strftime('%s', 'now'))
            ON CONFLICT(game_id) DO UPDATE SET
                keypair = excluded.keypair,
                wallet = excluded.wallet,
                active = 1,
                is_global = 1,
                created_at = COALESCE(created_at, strftime('%s', 'now'))
            "#,
        )
        .bind(game_id as i64)
        .bind(&encrypted[..])
        .bind(wallet_str)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert session (with keypair): {}", e);
            anyhow::anyhow!("Failed to insert session: {}", e)
        })?;
        Ok(())
    }

    fn decrypt_keypair_column(&self, blob: &[u8]) -> Option<[u8; 64]> {
        let bytes = match self.vault.decrypt_bytes(blob) {
            Ok(plaintext) => plaintext,
            Err(_) if blob.len() == 64 => blob.to_vec(),
            Err(e) => {
                tracing::error!("Failed to decrypt session keypair: {e}");
                return None;
            }
        };
        bytes.try_into().ok()
    }

    pub async fn get(&self, game_id: u64) -> Option<SessionEntry> {
        let row: (Vec<u8>, String, i64, i64) = sqlx::query_as(
            "SELECT keypair, wallet, active, is_global FROM sessions WHERE game_id = ?",
        )
        .bind(game_id as i64)
        .fetch_one(&self.pool)
        .await
        .ok()?;

        let keypair_bytes = self.decrypt_keypair_column(&row.0)?;

        let wallet_pubkey = Pubkey::from_str(&row.1).ok()?;
        let active = row.2 != 0;
        let is_global = row.3 != 0;

        Some(SessionEntry {
            keypair_bytes,
            wallet_pubkey,
            active,
            is_global,
        })
    }

    pub async fn activate(&self, game_id: u64) {
        sqlx::query("UPDATE sessions SET active = 1 WHERE game_id = ?")
            .bind(game_id as i64)
            .execute(&self.pool)
            .await
            .ok();
    }

    pub async fn deactivate(&self, game_id: u64) {
        sqlx::query("UPDATE sessions SET active = 0 WHERE game_id = ?")
            .bind(game_id as i64)
            .execute(&self.pool)
            .await
            .ok();
    }

    pub async fn list_active_game_ids(&self) -> Vec<u64> {
        sqlx::query_as::<_, (i64,)>("SELECT game_id FROM sessions WHERE active = 1")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(id,)| id as u64)
            .collect()
    }

    pub async fn wallet_activated(&self, game_id: u64, wallet: &Pubkey) -> bool {
        sqlx::query_as::<_, (i64,)>(
            "SELECT 1 FROM session_wallet_activations WHERE game_id = ? AND wallet = ?",
        )
        .bind(game_id as i64)
        .bind(wallet.to_string())
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .is_some()
    }

    pub async fn record_wallet_activation(&self, game_id: u64, wallet: &Pubkey, sig: &str) {
        sqlx::query(
            "INSERT OR IGNORE INTO session_wallet_activations (game_id, wallet, sig) VALUES (?1, ?2, ?3)",
        )
        .bind(game_id as i64)
        .bind(wallet.to_string())
        .bind(sig)
        .execute(&self.pool)
        .await
        .ok();
    }

    pub async fn is_active(&self, game_id: u64) -> bool {
        let (active,): (i64,) = sqlx::query_as("SELECT active FROM sessions WHERE game_id = ?")
            .bind(game_id as i64)
            .fetch_one(&self.pool)
            .await
            .unwrap_or((0,));
        active != 0
    }

    pub async fn count_active(&self) -> u64 {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE active = 1")
            .fetch_one(&self.pool)
            .await
            .unwrap_or((0,));
        count as u64
    }

    pub async fn count_unique_players(&self) -> u64 {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(DISTINCT wallet) FROM sessions")
            .fetch_one(&self.pool)
            .await
            .unwrap_or((0,));
        count as u64
    }

    pub async fn count_total_sessions(&self) -> u64 {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions")
            .fetch_one(&self.pool)
            .await
            .unwrap_or((0,));
        count as u64
    }

    pub async fn increment_move_count(&self, game_id: u64) -> i32 {
        let result: Result<(i64,), _> = sqlx::query_as(
            "UPDATE sessions SET move_count = move_count + 1 WHERE game_id = ? RETURNING move_count",
        )
        .bind(game_id as i64)
        .fetch_one(&self.pool)
        .await;
        result.map(|(n,)| n as i32).unwrap_or(1)
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signer::Signer;

    async fn test_store() -> SessionStore {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool");
        let vault = IdentityVault::new(&"1".repeat(64), &"2".repeat(64)).expect("test vault");
        let store = SessionStore::new(pool, vault);
        store.init().await.expect("init");
        store
    }

    #[tokio::test]
    async fn session_keypair_is_encrypted_at_rest() {
        let store = test_store().await;
        let wallet = Keypair::new().pubkey();
        let session_pubkey = store.create(1, wallet).await.expect("create session");

        // Read the raw column directly, bypassing `get()`'s decryption — this
        // is what a stolen SQLite backup would actually contain.
        let (raw,): (Vec<u8>,) = sqlx::query_as("SELECT keypair FROM sessions WHERE game_id = 1")
            .fetch_one(&store.pool)
            .await
            .expect("raw row");

        assert_ne!(
            raw.len(),
            64,
            "encrypted blob (nonce + ciphertext + AEAD tag) must not be the \
             same length as a raw plaintext keypair"
        );
        // The nonce/ciphertext framing means the plaintext pubkey bytes
        // shouldn't appear verbatim in the stored blob either.
        assert!(
            !raw.windows(32).any(|w| w == session_pubkey.to_bytes()),
            "raw stored bytes must not contain the plaintext pubkey"
        );

        // But the store's own decrypt path must still recover it correctly —
        // this is the round-trip the game's record_move flow depends on.
        let entry = store.get(1).await.expect("get session");
        assert_eq!(entry.session_pubkey(), session_pubkey);
    }

    #[tokio::test]
    async fn legacy_plaintext_keypair_rows_still_decode() {
        let store = test_store().await;
        let wallet = Keypair::new().pubkey();
        let kp = Keypair::new();
        let expected_pubkey = kp.pubkey();

        // Simulate a row written before at-rest encryption existed: raw
        // 64-byte plaintext, not a [nonce][ciphertext] blob.
        sqlx::query(
            "INSERT INTO sessions (game_id, keypair, wallet, active) VALUES (?1, ?2, ?3, 0)",
        )
        .bind(2i64)
        .bind(&kp.to_bytes()[..])
        .bind(wallet.to_string())
        .execute(&store.pool)
        .await
        .expect("insert legacy row");

        let entry = store
            .get(2)
            .await
            .expect("legacy plaintext row should still decode");
        assert_eq!(entry.session_pubkey(), expected_pubkey);
    }

    #[tokio::test]
    async fn create_with_keypair_is_also_encrypted_at_rest() {
        let store = test_store().await;
        let wallet = Keypair::new().pubkey();
        let kp = Keypair::new();
        let expected_pubkey = kp.pubkey();

        store
            .create_with_keypair(3, wallet, kp.to_bytes())
            .await
            .expect("create_with_keypair");

        let (raw,): (Vec<u8>,) = sqlx::query_as("SELECT keypair FROM sessions WHERE game_id = 3")
            .fetch_one(&store.pool)
            .await
            .expect("raw row");
        assert_ne!(
            raw.len(),
            64,
            "global-session keypair must be encrypted too"
        );

        let entry = store.get(3).await.expect("get session");
        assert_eq!(entry.session_pubkey(), expected_pubkey);
    }

    #[tokio::test]
    async fn create_with_keypair_rejects_repointing_an_active_sessions_wallet() {
        let store = test_store().await;
        let wallet_a = Keypair::new().pubkey();
        let wallet_b = Keypair::new().pubkey();
        let kp = Keypair::new();

        store
            .create_with_keypair(7, wallet_a, kp.to_bytes())
            .await
            .expect("first create_with_keypair should succeed");

        // Re-tracking the SAME wallet for the same game_id is a no-op retry
        // and must still succeed.
        store
            .create_with_keypair(7, wallet_a, kp.to_bytes())
            .await
            .expect("re-tracking the same wallet must still succeed");

        // A DIFFERENT wallet claiming the same, still-active game_id must
        // be rejected rather than silently repointing the session.
        let err = store
            .create_with_keypair(7, wallet_b, kp.to_bytes())
            .await
            .expect_err("a different wallet must not silently take over an active session");
        assert!(
            err.to_string().contains("different wallet"),
            "unexpected error: {err}"
        );

        // The session must still record the ORIGINAL wallet, not `wallet_b`.
        let (recorded_wallet,): (String,) =
            sqlx::query_as("SELECT wallet FROM sessions WHERE game_id = 7")
                .fetch_one(&store.pool)
                .await
                .expect("raw row");
        assert_eq!(recorded_wallet, wallet_a.to_string());
    }

    #[tokio::test]
    async fn create_with_keypair_sets_created_at_timestamp() {
        let store = test_store().await;
        let wallet = Keypair::new().pubkey();
        let kp = Keypair::new();

        store
            .create_with_keypair(11, wallet, kp.to_bytes())
            .await
            .expect("create_with_keypair should succeed");

        let (created_at,): (i64,) =
            sqlx::query_as("SELECT created_at FROM sessions WHERE game_id = 11")
                .fetch_one(&store.pool)
                .await
                .expect("created_at row");

        assert!(
            created_at > 0,
            "global-session tracked rows must get a real created_at timestamp"
        );
    }
}
