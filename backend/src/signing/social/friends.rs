//! Friend requests and contacts — ported from braid-reborn/server/src/chat/friends/.
//!
//! Primary identity key is the **Iroh node ID** (always available).
//! Solana pubkey is an optional second identifier that links into the ELO system.
//! Friends persist across wallet rotations because the social graph is node-ID anchored.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(rename_all = "snake_case")]
pub enum RequestStatus {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendRequest {
    pub id: String,
    pub from_node_id: String,
    pub from_pubkey: Option<String>,
    pub from_display: String,
    pub to_node_id: Option<String>,
    pub to_pubkey: Option<String>,
    pub message: Option<String>,
    pub status: RequestStatus,
    pub created_at: DateTime<Utc>,
    pub responded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub owner_node_id: String,
    pub contact_node_id: String,
    pub contact_pubkey: Option<String>,
    pub contact_display: String,
    pub contact_elo: Option<u16>,
    pub is_online: bool,
    pub last_seen: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct FriendManager {
    pool: SqlitePool,
}

impl FriendManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create tables if they don't exist yet (idempotent — called at startup).
    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS friend_requests (
                id           TEXT PRIMARY KEY,
                from_node_id TEXT NOT NULL,
                from_pubkey  TEXT,
                from_display TEXT NOT NULL,
                to_node_id   TEXT,
                to_pubkey    TEXT,
                message      TEXT,
                status       TEXT NOT NULL DEFAULT 'pending',
                created_at   TEXT NOT NULL,
                responded_at TEXT,
                UNIQUE(from_node_id, to_node_id),
                UNIQUE(from_node_id, to_pubkey)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS contacts (
                id               TEXT PRIMARY KEY,
                owner_node_id    TEXT NOT NULL,
                contact_node_id  TEXT NOT NULL,
                contact_pubkey   TEXT,
                contact_display  TEXT NOT NULL,
                contact_elo      INTEGER,
                created_at       TEXT NOT NULL,
                UNIQUE(owner_node_id, contact_node_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        info!("[Friends] Tables initialized");
        Ok(())
    }

    /// Send a friend request from `from_node_id` to the recipient identified
    /// by either `to_node_id` or `to_pubkey` (at least one must be set).
    pub async fn send_request(
        &self,
        from_node_id: String,
        from_pubkey: Option<String>,
        from_display: String,
        to_node_id: Option<String>,
        to_pubkey: Option<String>,
        message: Option<String>,
    ) -> Result<FriendRequest> {
        if to_node_id.is_none() && to_pubkey.is_none() {
            anyhow::bail!("must provide to_node_id or to_pubkey");
        }

        // Prevent duplicate pending
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM friend_requests WHERE from_node_id = ? AND (to_node_id = ? OR to_pubkey = ?) AND status = 'pending'",
        )
        .bind(&from_node_id)
        .bind(&to_node_id)
        .bind(&to_pubkey)
        .fetch_optional(&self.pool)
        .await?;

        if existing.is_some() {
            anyhow::bail!("friend request already pending");
        }

        let req = FriendRequest {
            id: Uuid::new_v4().to_string(),
            from_node_id: from_node_id.clone(),
            from_pubkey: from_pubkey.clone(),
            from_display: from_display.clone(),
            to_node_id: to_node_id.clone(),
            to_pubkey: to_pubkey.clone(),
            message: message.clone(),
            status: RequestStatus::Pending,
            created_at: Utc::now(),
            responded_at: None,
        };

        sqlx::query(
            "INSERT INTO friend_requests (id, from_node_id, from_pubkey, from_display, to_node_id, to_pubkey, message, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?)",
        )
        .bind(&req.id)
        .bind(&from_node_id)
        .bind(&from_pubkey)
        .bind(&from_display)
        .bind(&to_node_id)
        .bind(&to_pubkey)
        .bind(&message)
        .bind(req.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        info!("[Friends] Request {} → {:?}/{:?}", from_display, to_node_id, to_pubkey);
        Ok(req)
    }

    pub async fn get_pending_requests(&self, node_id: &str, pubkey: Option<&str>) -> Result<Vec<FriendRequest>> {
        let rows: Vec<(String, String, Option<String>, String, Option<String>, Option<String>, Option<String>, String, String)> =
            sqlx::query_as(
                r#"SELECT id, from_node_id, from_pubkey, from_display, to_node_id, to_pubkey, message, status, created_at
                   FROM friend_requests
                   WHERE (to_node_id = ? OR to_pubkey = ?) AND status = 'pending'
                   ORDER BY created_at DESC"#,
            )
            .bind(node_id)
            .bind(pubkey)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.into_iter().map(|(id, from_node, from_pk, from_disp, to_node, to_pk, msg, status, created)| {
            FriendRequest {
                id,
                from_node_id: from_node,
                from_pubkey: from_pk,
                from_display: from_disp,
                to_node_id: to_node,
                to_pubkey: to_pk,
                message: msg,
                status: match status.as_str() {
                    "accepted" => RequestStatus::Accepted,
                    "rejected" => RequestStatus::Rejected,
                    _          => RequestStatus::Pending,
                },
                created_at: created.parse().unwrap_or_else(|_| Utc::now()),
                responded_at: None,
            }
        }).collect())
    }

    pub async fn respond_to_request(
        &self,
        request_id: &str,
        accept: bool,
        responder_node_id: &str,
    ) -> Result<()> {
        let row: (String, String, Option<String>, String, Option<String>, Option<String>, String) =
            sqlx::query_as(
                "SELECT id, from_node_id, from_pubkey, from_display, to_node_id, to_pubkey, status FROM friend_requests WHERE id = ?",
            )
            .bind(request_id)
            .fetch_one(&self.pool)
            .await?;

        let (_, from_node, from_pk, from_disp, _, to_pk, status) = row;

        if status != "pending" {
            anyhow::bail!("request already responded to");
        }

        let new_status = if accept { "accepted" } else { "rejected" };
        let now = Utc::now();

        sqlx::query("UPDATE friend_requests SET status = ?, responded_at = ? WHERE id = ?")
            .bind(new_status)
            .bind(now.to_rfc3339())
            .bind(request_id)
            .execute(&self.pool)
            .await?;

        if accept {
            // Create bidirectional contact entries
            let now_str = now.to_rfc3339();
            let responder_display = responder_node_id; // fallback; caller can override later
            sqlx::query(
                "INSERT OR IGNORE INTO contacts (id, owner_node_id, contact_node_id, contact_pubkey, contact_display, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&from_node)
            .bind(responder_node_id)
            .bind(&to_pk)
            .bind(responder_display)
            .bind(&now_str)
            .execute(&self.pool)
            .await?;

            sqlx::query(
                "INSERT OR IGNORE INTO contacts (id, owner_node_id, contact_node_id, contact_pubkey, contact_display, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(responder_node_id)
            .bind(&from_node)
            .bind(&from_pk)
            .bind(&from_disp)
            .bind(&now_str)
            .execute(&self.pool)
            .await?;

            info!("[Friends] Request {} accepted, contacts created", request_id);
        }

        Ok(())
    }

    pub async fn get_contacts(&self, owner_node_id: &str) -> Result<Vec<Contact>> {
        let rows: Vec<(String, String, Option<String>, String, Option<i64>, String)> =
            sqlx::query_as(
                r#"SELECT id, contact_node_id, contact_pubkey, contact_display, contact_elo, created_at
                   FROM contacts WHERE owner_node_id = ? ORDER BY contact_display"#,
            )
            .bind(owner_node_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.into_iter().map(|(id, node, pk, disp, elo, created)| Contact {
            id,
            owner_node_id: owner_node_id.to_string(),
            contact_node_id: node,
            contact_pubkey: pk,
            contact_display: disp,
            contact_elo: elo.map(|e| e as u16),
            is_online: false,
            last_seen: None,
            created_at: created.parse().unwrap_or_else(|_| Utc::now()),
        }).collect())
    }

    pub async fn remove_contact(&self, owner_node_id: &str, contact_node_id: &str) -> Result<()> {
        sqlx::query(
            "DELETE FROM contacts WHERE (owner_node_id = ? AND contact_node_id = ?) OR (owner_node_id = ? AND contact_node_id = ?)",
        )
        .bind(owner_node_id)
        .bind(contact_node_id)
        .bind(contact_node_id)
        .bind(owner_node_id)
        .execute(&self.pool)
        .await?;
        info!("[Friends] Contact removed: {} <-> {}", owner_node_id, contact_node_id);
        Ok(())
    }
}
