//! Persistent session storage for disconnect recovery

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

/// Session status for reconnect handling
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Active,
    Paused,
    Resumable,
    Expired,
}

impl ToString for SessionStatus {
    fn to_string(&self) -> String {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Paused => "paused",
            SessionStatus::Resumable => "resumable",
            SessionStatus::Expired => "expired",
        }
        .to_string()
    }
}

impl From<String> for SessionStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "active" => SessionStatus::Active,
            "paused" => SessionStatus::Paused,
            "resumable" => SessionStatus::Resumable,
            _ => SessionStatus::Expired,
        }
    }
}

/// Active game session for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSession {
    pub session_id: String,
    pub game_id: u64,
    pub player_white: String,
    pub player_black: String,
    pub current_fen: String,
    pub move_history: Vec<String>,
    pub white_time_ms: i64,
    pub black_time_ms: i64,
    pub last_activity: i64,
    pub grace_period_ends: i64,
    pub status: SessionStatus,
}

/// Session store for SQLite operations
#[derive(Clone)]
pub struct SessionStore {
    pool: SqlitePool,
}

impl SessionStore {
    /// Create a new session store
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Save or update a session
    pub async fn save_session(&self, session: &ActiveSession) -> Result<()> {
        let move_history_json = serde_json::to_string(&session.move_history)?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO active_sessions
            (session_id, game_id, player_white, player_black, current_fen,
             move_history, white_time_ms, black_time_ms, last_activity,
             grace_period_ends, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
        )
        .bind(&session.session_id)
        .bind(session.game_id as i64)
        .bind(&session.player_white)
        .bind(&session.player_black)
        .bind(&session.current_fen)
        .bind(move_history_json)
        .bind(session.white_time_ms)
        .bind(session.black_time_ms)
        .bind(session.last_activity)
        .bind(session.grace_period_ends)
        .bind(session.status.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a resumable session for a player
    pub async fn get_resumable_session(
        &self,
        player_pubkey: &str,
    ) -> Result<Option<ActiveSession>> {
        let now = chrono::Utc::now().timestamp();

        let row = sqlx::query(
            r#"
            SELECT * FROM active_sessions
            WHERE (player_white = ?1 OR player_black = ?1)
            AND status = 'resumable'
            AND grace_period_ends > ?2
            ORDER BY last_activity DESC
            LIMIT 1
            "#,
        )
        .bind(player_pubkey)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(self.row_to_session(r)?)),
            None => Ok(None),
        }
    }

    /// Get session by game ID
    pub async fn get_session_by_game(&self, game_id: u64) -> Result<Option<ActiveSession>> {
        let row =
            sqlx::query("SELECT * FROM active_sessions WHERE game_id = ?1 AND status != 'expired'")
                .bind(game_id as i64)
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some(r) => Ok(Some(self.row_to_session(r)?)),
            None => Ok(None),
        }
    }

    /// Update session status
    pub async fn update_status(
        &self,
        session_id: &str,
        status: SessionStatus,
        grace_period_ends: Option<i64>,
    ) -> Result<()> {
        let mut query = String::from("UPDATE active_sessions SET status = ?1");
        if grace_period_ends.is_some() {
            query.push_str(", grace_period_ends = ?2");
        }
        query.push_str(" WHERE session_id = ?");

        // SAFETY: `query` only ever grows by appending the static literal
        // ", grace_period_ends = ?2" above; every actual value is bound, never
        // interpolated. No user input reaches the SQL text itself.
        let mut q = sqlx::query(sqlx::AssertSqlSafe(query)).bind(status.to_string());

        if let Some(gpe) = grace_period_ends {
            q = q.bind(gpe);
        }

        q.bind(session_id).execute(&self.pool).await?;

        Ok(())
    }

    /// Mark session as expired (cleanup)
    pub async fn expire_session(&self, session_id: &str) -> Result<()> {
        self.update_status(session_id, SessionStatus::Expired, None)
            .await
    }

    /// Delete old expired sessions
    pub async fn cleanup_expired(&self, days_old: u32) -> Result<u64> {
        let cutoff = chrono::Utc::now().timestamp() - (days_old as i64 * 24 * 60 * 60);

        let result = sqlx::query(
            "DELETE FROM active_sessions WHERE status = 'expired' AND last_activity < ?1",
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Convert database row to ActiveSession
    fn row_to_session(&self, row: sqlx::sqlite::SqliteRow) -> Result<ActiveSession> {
        let move_history_json: String = row.try_get("move_history")?;
        let move_history: Vec<String> = serde_json::from_str(&move_history_json)?;

        Ok(ActiveSession {
            session_id: row.try_get("session_id")?,
            game_id: row.try_get::<i64, _>("game_id")? as u64,
            player_white: row.try_get("player_white")?,
            player_black: row.try_get("player_black")?,
            current_fen: row.try_get("current_fen")?,
            move_history,
            white_time_ms: row.try_get("white_time_ms")?,
            black_time_ms: row.try_get("black_time_ms")?,
            last_activity: row.try_get("last_activity")?,
            grace_period_ends: row.try_get("grace_period_ends")?,
            status: SessionStatus::from(row.try_get::<String, _>("status")?),
        })
    }
}
