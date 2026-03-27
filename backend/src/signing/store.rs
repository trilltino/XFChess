use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use sqlx::SqlitePool;
use std::str::FromStr;

#[derive(Clone)]
pub struct SessionEntry {
    pub keypair_bytes: [u8; 64],
    pub wallet_pubkey: Pubkey,
    pub active: bool,
}

impl SessionEntry {
    pub fn keypair(&self) -> Keypair {
        Keypair::try_from(self.keypair_bytes.as_slice()).expect("valid keypair bytes")
    }

    pub fn session_pubkey(&self) -> Pubkey {
        self.keypair().pubkey()
    }
}

/// SQLite-backed session store. Persists across server restarts.
#[derive(Clone)]
pub struct SessionStore {
    pool: SqlitePool,
}

impl SessionStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the sessions table if it doesn't exist.
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
        Ok(())
    }

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

    pub async fn activate(&self, game_id: u64) {
        sqlx::query("UPDATE sessions SET active = 1 WHERE game_id = ?")
            .bind(game_id as i64)
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
}
