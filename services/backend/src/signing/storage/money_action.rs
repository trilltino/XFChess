use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MoneyActionRecord {
    pub id: String,
    pub action_type: String,
    pub scope_type: String,
    pub game_id: Option<i64>,
    pub tournament_id: Option<i64>,
    pub wallet: Option<String>,
    pub signature: Option<String>,
    pub status: String,
    pub reason: Option<String>,
    pub attempt_count: i64,
    pub last_error: Option<String>,
    pub next_retry_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewMoneyAction {
    pub action_type: String,
    pub scope_type: String,
    pub game_id: Option<i64>,
    pub tournament_id: Option<i64>,
    pub wallet: Option<String>,
    pub signature: Option<String>,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MoneyActionStore {
    pool: SqlitePool,
}

impl MoneyActionStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS money_actions (
                id TEXT PRIMARY KEY,
                action_type TEXT NOT NULL,
                scope_type TEXT NOT NULL,
                game_id INTEGER,
                tournament_id INTEGER,
                wallet TEXT,
                signature TEXT,
                status TEXT NOT NULL,
                reason TEXT,
                attempt_count INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                next_retry_at INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_money_actions_scope \
             ON money_actions(scope_type, game_id, tournament_id, wallet)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_money_actions_status_retry \
             ON money_actions(status, next_retry_at, updated_at)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_money_actions_signature \
             ON money_actions(signature)",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_or_get(
        &self,
        input: NewMoneyAction,
    ) -> Result<MoneyActionRecord, sqlx::Error> {
        if let Some(signature) = input.signature.as_deref().filter(|s| !s.is_empty()) {
            if let Some(existing) = sqlx::query_as::<_, MoneyActionRecord>(
                "SELECT * FROM money_actions WHERE action_type = ? AND signature = ? LIMIT 1",
            )
            .bind(&input.action_type)
            .bind(signature)
            .fetch_optional(&self.pool)
            .await?
            {
                return Ok(existing);
            }
        }

        let now = Utc::now().timestamp();
        let id = format!("ma_{}", uuid::Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO money_actions (
                id, action_type, scope_type, game_id, tournament_id, wallet, signature,
                status, reason, attempt_count, last_error, next_retry_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, NULL, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.action_type)
        .bind(&input.scope_type)
        .bind(input.game_id)
        .bind(input.tournament_id)
        .bind(input.wallet)
        .bind(input.signature)
        .bind(input.status)
        .bind(input.reason)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.get(&id).await?.ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn get(&self, id: &str) -> Result<Option<MoneyActionRecord>, sqlx::Error> {
        sqlx::query_as::<_, MoneyActionRecord>("SELECT * FROM money_actions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn by_scope(
        &self,
        game_id: Option<i64>,
        tournament_id: Option<i64>,
        wallet: Option<&str>,
    ) -> Result<Vec<MoneyActionRecord>, sqlx::Error> {
        sqlx::query_as::<_, MoneyActionRecord>(
            r#"
            SELECT * FROM money_actions
            WHERE (? IS NULL OR game_id = ?)
              AND (? IS NULL OR tournament_id = ?)
              AND (? IS NULL OR wallet = ?)
            ORDER BY updated_at DESC, created_at DESC
            LIMIT 100
            "#,
        )
        .bind(game_id)
        .bind(game_id)
        .bind(tournament_id)
        .bind(tournament_id)
        .bind(wallet)
        .bind(wallet)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn retryable(&self, limit: i64) -> Result<Vec<MoneyActionRecord>, sqlx::Error> {
        let now = Utc::now().timestamp();
        sqlx::query_as::<_, MoneyActionRecord>(
            r#"
            SELECT * FROM money_actions
            WHERE status IN ('submitted', 'pending_reconciliation', 'confirmed')
              AND (next_retry_at IS NULL OR next_retry_at <= ?)
            ORDER BY updated_at ASC
            LIMIT ?
            "#,
        )
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn transition(
        &self,
        id: &str,
        status: &str,
        reason: Option<&str>,
        last_error: Option<&str>,
        next_retry_at: Option<i64>,
        increment_attempt: bool,
    ) -> Result<Option<MoneyActionRecord>, sqlx::Error> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE money_actions
            SET status = ?,
                reason = ?,
                last_error = ?,
                next_retry_at = ?,
                attempt_count = attempt_count + ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(reason)
        .bind(last_error)
        .bind(next_retry_at)
        .bind(if increment_attempt { 1_i64 } else { 0_i64 })
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get(id).await
    }
}
