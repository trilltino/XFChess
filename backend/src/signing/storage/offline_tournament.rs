use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineTournamentRecord {
    pub tournament_id: String,
    pub name: String,
    pub format: String,
    pub status: String,
    pub state: serde_json::Value,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone)]
pub struct OfflineTournamentStore {
    pool: SqlitePool,
}

impl OfflineTournamentStore {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn create(&self, record: &OfflineTournamentRecord) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO offline_tournaments (tournament_id, name, format, status, state_json, revision, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&record.tournament_id).bind(&record.name).bind(&record.format)
            .bind(&record.status).bind(serde_json::to_string(&record.state).unwrap_or_else(|_| "{}".to_string()))
            .bind(record.revision).bind(record.created_at).bind(record.updated_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<OfflineTournamentRecord>, sqlx::Error> {
        let row = sqlx::query("SELECT tournament_id, name, format, status, state_json, revision, created_at, updated_at FROM offline_tournaments WHERE tournament_id = ?")
            .bind(id).fetch_optional(&self.pool).await?;
        Ok(row.map(|r| OfflineTournamentRecord {
            tournament_id: r.get("tournament_id"), name: r.get("name"), format: r.get("format"), status: r.get("status"),
            state: serde_json::from_str(r.get::<String, _>("state_json").as_str()).unwrap_or(serde_json::json!({})),
            revision: r.get("revision"), created_at: r.get("created_at"), updated_at: r.get("updated_at"),
        }))
    }

    pub async fn list(&self) -> Result<Vec<OfflineTournamentRecord>, sqlx::Error> {
        let rows = sqlx::query("SELECT tournament_id, name, format, status, state_json, revision, created_at, updated_at FROM offline_tournaments ORDER BY updated_at DESC")
            .fetch_all(&self.pool).await?;
        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get("tournament_id");
            if let Some(record) = self.get(&id).await? { records.push(record); }
        }
        Ok(records)
    }

    pub async fn update_state(&self, id: &str, status: &str, state: &serde_json::Value, revision: i64, updated_at: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE offline_tournaments SET status = ?, state_json = ?, revision = ?, updated_at = ? WHERE tournament_id = ? AND revision = ?")
            .bind(status).bind(serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string())).bind(revision).bind(updated_at).bind(id).bind(revision - 1).execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }
}
