use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
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
    /// How the sender knows the addressee: a username, else the name last seen for them.
    #[serde(default)]
    pub to_display: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedPlayer {
    pub blocked_node_id: String,
    pub blocked_pubkey: Option<String>,
    pub blocked_display: String,
    pub created_at: DateTime<Utc>,
}

pub struct FriendManager {
    pool: SqlitePool,
}

fn ordered<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Whether `blocker` (a node, or that player's wallet) has blocked `blocked` (a node or wallet).
async fn is_blocked(
    conn: &mut SqliteConnection,
    blocker: (&str, Option<&str>),
    blocked: (&str, Option<&str>),
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM blocks
         WHERE (blocker_node_id = ? OR (? IS NOT NULL AND blocker_pubkey = ?))
           AND (blocked_node_id = ? OR (? IS NOT NULL AND blocked_pubkey = ?)))",
    )
    .bind(blocker.0)
    .bind(blocker.1)
    .bind(blocker.1)
    .bind(blocked.0)
    .bind(blocked.1)
    .bind(blocked.1)
    .fetch_one(conn)
    .await?)
}

async fn blocked_either_way(
    conn: &mut SqliteConnection,
    a: (&str, Option<&str>),
    b: (&str, Option<&str>),
) -> Result<bool> {
    Ok(is_blocked(&mut *conn, a, b).await? || is_blocked(&mut *conn, b, a).await?)
}

/// The wallet `node` is known to play as: the one on a contact row, else the last verified one.
async fn wallet_for(
    conn: &mut SqliteConnection,
    owner: &str,
    node: &str,
) -> Result<Option<String>> {
    let from_contact: Option<String> = sqlx::query_scalar(
        "SELECT contact_pubkey FROM contacts WHERE owner_node_id = ? AND contact_node_id = ? AND contact_pubkey IS NOT NULL",
    )
    .bind(owner)
    .bind(node)
    .fetch_optional(&mut *conn)
    .await?;
    if from_contact.is_some() {
        return Ok(from_contact);
    }
    Ok(
        sqlx::query_scalar("SELECT wallet FROM wallet_nodes WHERE node_id = ?")
            .bind(node)
            .fetch_optional(conn)
            .await?,
    )
}

type RequestRow = (
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<String>,
);
type ContactRow = (String, String, Option<String>, String, Option<i64>, String);

impl FriendManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

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
        if to_node_id.as_deref() == Some(&from_node_id)
            || from_pubkey
                .as_ref()
                .is_some_and(|pk| Some(pk) == to_pubkey.as_ref())
        {
            anyhow::bail!("You cannot add yourself");
        }
        let mut transaction = self.pool.begin().await?;
        let recipient_node = to_node_id.as_deref().unwrap_or("");
        let recipient_wallet = match (&to_pubkey, &to_node_id) {
            (Some(wallet), _) => Some(wallet.clone()),
            (None, Some(node)) => wallet_for(&mut transaction, &from_node_id, node).await?,
            (None, None) => None,
        };
        let sender = (from_node_id.as_str(), from_pubkey.as_deref());
        let recipient = (recipient_node, recipient_wallet.as_deref());
        if is_blocked(&mut transaction, recipient, sender).await? {
            anyhow::bail!("This player cannot receive your request");
        }
        if is_blocked(&mut transaction, sender, recipient).await? {
            anyhow::bail!("Unblock this player before sending a request");
        }
        if let Some(target) = &to_node_id {
            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM contacts WHERE owner_node_id = ? AND contact_node_id = ?)")
                .bind(&from_node_id).bind(target).fetch_one(&mut *transaction).await?;
            if exists {
                anyhow::bail!("You are already friends");
            }
        }

        if let (Some(mine), Some(theirs)) = (from_pubkey.as_deref(), to_pubkey.as_deref()) {
            let (lo, hi) = ordered(mine, theirs);
            let friends: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM wallet_friendships WHERE wallet_lo = ? AND wallet_hi = ?)",
            )
            .bind(lo)
            .bind(hi)
            .fetch_one(&mut *transaction)
            .await?;
            if friends {
                anyhow::bail!("You are already friends");
            }
        }

        // The addressee's name: an account username, else the name last seen for the wallet or node.
        let to_display: Option<String> = match (&to_pubkey, &to_node_id) {
            (Some(wallet), _) => {
                let username: Option<String> = sqlx::query_scalar(
                    "SELECT username FROM users WHERE wallet = ? AND deleted_at IS NULL",
                )
                .bind(wallet)
                .fetch_optional(&mut *transaction)
                .await?;
                match username {
                    Some(name) => Some(name),
                    None => {
                        sqlx::query_scalar("SELECT display FROM wallet_nodes WHERE wallet = ?")
                            .bind(wallet)
                            .fetch_optional(&mut *transaction)
                            .await?
                    }
                }
            }
            (None, Some(node)) => {
                sqlx::query_scalar("SELECT display FROM wallet_nodes WHERE node_id = ?")
                    .bind(node)
                    .fetch_optional(&mut *transaction)
                    .await?
            }
            (None, None) => None,
        };

        // Prevent duplicate pending
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM friend_requests WHERE from_node_id = ? AND (to_node_id = ? OR to_pubkey = ?) AND status = 'pending'",
        )
        .bind(&from_node_id)
        .bind(&to_node_id)
        .bind(&to_pubkey)
        .fetch_optional(&mut *transaction)
        .await?;

        if existing.is_some() {
            anyhow::bail!("friend request already pending");
        }

        // Historical rejected/accepted rows must not prevent re-adding a removed friend.
        sqlx::query("DELETE FROM friend_requests WHERE from_node_id = ? AND (to_node_id = ? OR to_pubkey = ?) AND status != 'pending'")
            .bind(&from_node_id).bind(&to_node_id).bind(&to_pubkey).execute(&mut *transaction).await?;

        let req = FriendRequest {
            id: Uuid::new_v4().to_string(),
            from_node_id: from_node_id.clone(),
            from_pubkey: from_pubkey.clone(),
            from_display: from_display.clone(),
            to_node_id: to_node_id.clone(),
            to_pubkey: to_pubkey.clone(),
            to_display: to_display.clone(),
            message: message.clone(),
            status: RequestStatus::Pending,
            created_at: Utc::now(),
            responded_at: None,
        };

        sqlx::query(
            "INSERT INTO friend_requests (id, from_node_id, from_pubkey, from_display, to_node_id, to_pubkey, message, status, created_at, to_display)
             VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)",
        )
        .bind(&req.id)
        .bind(&from_node_id)
        .bind(&from_pubkey)
        .bind(&from_display)
        .bind(&to_node_id)
        .bind(&to_pubkey)
        .bind(&message)
        .bind(req.created_at.to_rfc3339())
        .bind(&to_display)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        info!(
            "[Friends] Request {} → {:?}/{:?}",
            from_display, to_node_id, to_pubkey
        );
        Ok(req)
    }

    pub async fn get_pending_requests(
        &self,
        node_id: &str,
        pubkey: Option<&str>,
    ) -> Result<Vec<FriendRequest>> {
        let rows: Vec<RequestRow> =
            sqlx::query_as(
                r#"SELECT id, from_node_id, from_pubkey, from_display, to_node_id, to_pubkey, message, status, created_at, to_display
                   FROM friend_requests
                   WHERE (to_node_id = ? OR to_pubkey = ?) AND status = 'pending'
                   ORDER BY created_at DESC"#,
            )
            .bind(node_id)
            .bind(pubkey)
            .fetch_all(&self.pool)
            .await?;

        Ok(decode_requests(rows))
    }

    /// Whether `node_id` (or the verified `wallet`) is who the request was sent to.
    pub async fn is_addressee(
        &self,
        request_id: &str,
        node_id: &str,
        wallet: Option<&str>,
    ) -> Result<bool> {
        let (to_node, to_pubkey): (Option<String>, Option<String>) =
            sqlx::query_as("SELECT to_node_id, to_pubkey FROM friend_requests WHERE id = ?")
                .bind(request_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(to_node.as_deref() == Some(node_id)
            || matches!((to_pubkey.as_deref(), wallet), (Some(to), Some(w)) if to == w))
    }

    pub async fn respond_to_request(
        &self,
        request_id: &str,
        accept: bool,
        responder_node_id: &str,
        responder_display: &str,
        responder_wallet: Option<&str>,
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        let row: (String, String, Option<String>, String, Option<String>, Option<String>, String) =
            sqlx::query_as(
                "SELECT id, from_node_id, from_pubkey, from_display, to_node_id, to_pubkey, status FROM friend_requests WHERE id = ?",
            )
            .bind(request_id)
            .fetch_one(&mut *transaction)
            .await?;

        let (_, from_node, from_pk, from_disp, _, to_pk, status) = row;

        if status != "pending" {
            anyhow::bail!("request already responded to");
        }

        let responder_wallet = responder_wallet.or(to_pk.as_deref());
        if accept
            && blocked_either_way(
                &mut transaction,
                (&from_node, from_pk.as_deref()),
                (responder_node_id, responder_wallet),
            )
            .await?
        {
            anyhow::bail!("This request is no longer available");
        }

        let new_status = if accept { "accepted" } else { "rejected" };
        let now = Utc::now();

        let changed = sqlx::query("UPDATE friend_requests SET status = ?, responded_at = ? WHERE id = ? AND status = 'pending'")
            .bind(new_status)
            .bind(now.to_rfc3339())
            .bind(request_id)
            .execute(&mut *transaction)
            .await?;
        if changed.rows_affected() != 1 {
            anyhow::bail!("request already responded to");
        }

        if accept {
            // Crossed requests describe the same relationship, not a second
            // pending request after both contacts have been created.
            sqlx::query("UPDATE friend_requests SET status = 'accepted', responded_at = ? WHERE from_node_id = ? AND to_node_id = ? AND status = 'pending'")
                .bind(now.to_rfc3339()).bind(responder_node_id).bind(&from_node).execute(&mut *transaction).await?;
            let now_str = now.to_rfc3339();
            sqlx::query(
                "INSERT OR IGNORE INTO contacts (id, owner_node_id, contact_node_id, contact_pubkey, contact_display, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&from_node)
            .bind(responder_node_id)
            .bind(responder_wallet)
            .bind(responder_display)
            .bind(&now_str)
            .execute(&mut *transaction)
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
            .execute(&mut *transaction)
            .await?;

            // Two verified wallets stay friends across devices; the node-keyed contacts
            // are re-derived from this row (see `sync_wallet_contacts`).
            if let (Some(a), Some(b)) = (from_pk.as_deref(), responder_wallet) {
                if a != b {
                    let (lo, hi) = ordered(a, b);
                    sqlx::query("INSERT OR IGNORE INTO wallet_friendships (wallet_lo, wallet_hi, created_at) VALUES (?, ?, ?)")
                        .bind(lo).bind(hi).bind(&now_str).execute(&mut *transaction).await?;
                }
            }

            info!(
                "[Friends] Request {} accepted, contacts created",
                request_id
            );
        }

        transaction.commit().await?;
        Ok(())
    }

    pub async fn get_contacts(&self, owner_node_id: &str) -> Result<Vec<Contact>> {
        let rows: Vec<ContactRow> = sqlx::query_as(
            r#"SELECT id, contact_node_id, contact_pubkey, contact_display, contact_elo, created_at
                   FROM contacts WHERE owner_node_id = ? ORDER BY contact_display"#,
        )
        .bind(owner_node_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, node, pk, disp, elo, created)| Contact {
                id,
                owner_node_id: owner_node_id.to_string(),
                contact_node_id: node,
                contact_pubkey: pk,
                contact_display: disp,
                contact_elo: elo.map(|e| e as u16),
                is_online: false,
                last_seen: None,
                created_at: created.parse().unwrap_or_else(|_| Utc::now()),
            })
            .collect())
    }

    pub async fn remove_contact(
        &self,
        owner_node_id: &str,
        contact_node_id: &str,
        owner_wallet: Option<&str>,
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        let mine = match owner_wallet {
            Some(wallet) => Some(wallet.to_string()),
            None => wallet_for(&mut transaction, contact_node_id, owner_node_id).await?,
        };
        let theirs = wallet_for(&mut transaction, owner_node_id, contact_node_id).await?;
        if let (Some(a), Some(b)) = (mine.as_deref(), theirs.as_deref()) {
            let (lo, hi) = ordered(a, b);
            sqlx::query("DELETE FROM wallet_friendships WHERE wallet_lo = ? AND wallet_hi = ?")
                .bind(lo)
                .bind(hi)
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query(
            "DELETE FROM contacts WHERE (owner_node_id = ? AND contact_node_id = ?) OR (owner_node_id = ? AND contact_node_id = ?)",
        )
        .bind(owner_node_id)
        .bind(contact_node_id)
        .bind(contact_node_id)
        .bind(owner_node_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM social_invites WHERE (recipient = ? AND sender = ?) OR (recipient = ? AND sender = ?)")
            .bind(owner_node_id).bind(contact_node_id).bind(contact_node_id).bind(owner_node_id)
            .execute(&mut *transaction).await?;
        transaction.commit().await?;
        info!(
            "[Friends] Contact removed: {} <-> {}",
            owner_node_id, contact_node_id
        );
        Ok(())
    }

    pub async fn outgoing_requests(&self, owner: &str) -> Result<Vec<FriendRequest>> {
        let rows: Vec<RequestRow> = sqlx::query_as(
            "SELECT id, from_node_id, from_pubkey, from_display, to_node_id, to_pubkey, message, status, created_at, to_display FROM friend_requests WHERE from_node_id = ? AND status = 'pending' ORDER BY created_at DESC"
        ).bind(owner).fetch_all(&self.pool).await?;
        Ok(decode_requests(rows))
    }

    pub async fn cancel_request(&self, id: &str, owner: &str) -> Result<bool> {
        Ok(sqlx::query(
            "DELETE FROM friend_requests WHERE id = ? AND from_node_id = ? AND status = 'pending'",
        )
        .bind(id)
        .bind(owner)
        .execute(&self.pool)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn are_friends(&self, owner: &str, target: &str) -> Result<bool> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM contacts WHERE owner_node_id = ? AND contact_node_id = ?)",
        )
        .bind(owner)
        .bind(target)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Records the node a verified wallet is currently playing from.
    pub async fn note_wallet_node(&self, wallet: &str, node_id: &str, display: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO wallet_nodes (wallet, node_id, display, updated_at) VALUES (?, ?, ?, ?)
             ON CONFLICT(wallet) DO UPDATE SET node_id = excluded.node_id, display = excluded.display, updated_at = excluded.updated_at",
        )
        .bind(wallet)
        .bind(node_id)
        .bind(display)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The wallet registered under `username` (case-insensitive, exact match only).
    /// Accounts that were erased, or that registered by email and have no wallet yet, never match.
    pub async fn resolve_username(&self, username: &str) -> Result<Option<(String, String)>> {
        Ok(sqlx::query_as(
            "SELECT wallet, username FROM users
             WHERE LOWER(username) = LOWER(?) AND wallet != '' AND deleted_at IS NULL",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// The verified wallet last seen on `node_id`, if any.
    pub async fn wallet_of_node(&self, node_id: &str) -> Result<Option<String>> {
        Ok(
            sqlx::query_scalar("SELECT wallet FROM wallet_nodes WHERE node_id = ?")
                .bind(node_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// Whether two verified wallets accepted each other as friends.
    pub async fn wallets_are_friends(&self, a: &str, b: &str) -> Result<bool> {
        if a == b {
            return Ok(false);
        }
        let (lo, hi) = ordered(a, b);
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM wallet_friendships WHERE wallet_lo = ? AND wallet_hi = ?)",
        )
        .bind(lo)
        .bind(hi)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Whether either side has blocked the other.
    pub async fn blocked_between(
        &self,
        a: (&str, Option<&str>),
        b: (&str, Option<&str>),
    ) -> Result<bool> {
        let mut conn = self.pool.acquire().await?;
        blocked_either_way(&mut conn, a, b).await
    }

    /// Makes a wallet's friendships visible to the node it is playing from: contacts for
    /// friends it has not met on this device are created, and routes to friends that moved to
    /// another device are refreshed. Blocked players are skipped.
    pub async fn sync_wallet_contacts(&self, owner_node: &str, wallet: &str) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        let others: Vec<String> = sqlx::query_scalar(
            "SELECT CASE WHEN wallet_lo = ? THEN wallet_hi ELSE wallet_lo END
             FROM wallet_friendships WHERE wallet_lo = ? OR wallet_hi = ?",
        )
        .bind(wallet)
        .bind(wallet)
        .bind(wallet)
        .fetch_all(&mut *transaction)
        .await?;
        let now = Utc::now().to_rfc3339();
        for other in others {
            let node: Option<(String, String)> =
                sqlx::query_as("SELECT node_id, display FROM wallet_nodes WHERE wallet = ?")
                    .bind(&other)
                    .fetch_optional(&mut *transaction)
                    .await?;
            let Some((node, display)) = node else {
                continue;
            };
            if node == owner_node
                || blocked_either_way(
                    &mut transaction,
                    (owner_node, Some(wallet)),
                    (&node, Some(&other)),
                )
                .await?
            {
                continue;
            }
            let known: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM contacts WHERE owner_node_id = ? AND contact_pubkey = ?)",
            )
            .bind(owner_node)
            .bind(&other)
            .fetch_one(&mut *transaction)
            .await?;
            if known {
                sqlx::query("UPDATE OR IGNORE contacts SET contact_node_id = ? WHERE owner_node_id = ? AND contact_pubkey = ? AND contact_node_id != ?")
                    .bind(&node).bind(owner_node).bind(&other).bind(&node).execute(&mut *transaction).await?;
            } else {
                sqlx::query("INSERT OR IGNORE INTO contacts (id, owner_node_id, contact_node_id, contact_pubkey, contact_display, created_at) VALUES (?, ?, ?, ?, ?, ?)")
                    .bind(Uuid::new_v4().to_string()).bind(owner_node).bind(&node).bind(&other).bind(&display).bind(&now)
                    .execute(&mut *transaction).await?;
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Blocks `target`: the pair stops being friends and neither request, invitation nor
    /// accept can pass between them until the block is lifted.
    pub async fn block(
        &self,
        owner: &str,
        owner_wallet: Option<&str>,
        target: &str,
    ) -> Result<()> {
        if owner == target {
            anyhow::bail!("You cannot block yourself");
        }
        let mut transaction = self.pool.begin().await?;
        let owner_wallet: Option<String> = match owner_wallet {
            Some(wallet) => Some(wallet.to_string()),
            None => sqlx::query_scalar("SELECT wallet FROM wallet_nodes WHERE node_id = ?")
                .bind(owner)
                .fetch_optional(&mut *transaction)
                .await?,
        };
        let target_wallet = match wallet_for(&mut transaction, owner, target).await? {
            Some(wallet) => Some(wallet),
            None => sqlx::query_scalar("SELECT from_pubkey FROM friend_requests WHERE from_node_id = ? AND to_node_id = ? AND from_pubkey IS NOT NULL LIMIT 1")
                .bind(target).bind(owner).fetch_optional(&mut *transaction).await?,
        };
        let known_display: Option<String> = sqlx::query_scalar("SELECT contact_display FROM contacts WHERE owner_node_id = ? AND contact_node_id = ?")
            .bind(owner).bind(target).fetch_optional(&mut *transaction).await?;
        let display = match known_display {
            Some(display) => display,
            None => sqlx::query_scalar("SELECT display FROM wallet_nodes WHERE node_id = ?")
                .bind(target)
                .fetch_optional(&mut *transaction)
                .await?
                .unwrap_or_else(|| target.to_string()),
        };
        sqlx::query(
            "INSERT INTO blocks (blocker_node_id, blocked_node_id, blocker_pubkey, blocked_pubkey, blocked_display, created_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(blocker_node_id, blocked_node_id) DO UPDATE SET
               blocker_pubkey = COALESCE(excluded.blocker_pubkey, blocker_pubkey),
               blocked_pubkey = COALESCE(excluded.blocked_pubkey, blocked_pubkey)",
        )
        .bind(owner)
        .bind(target)
        .bind(&owner_wallet)
        .bind(&target_wallet)
        .bind(&display)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM contacts WHERE (owner_node_id = ? AND contact_node_id = ?) OR (owner_node_id = ? AND contact_node_id = ?)")
            .bind(owner).bind(target).bind(target).bind(owner).execute(&mut *transaction).await?;
        sqlx::query("DELETE FROM friend_requests WHERE (from_node_id = ? AND (to_node_id = ? OR (? IS NOT NULL AND to_pubkey = ?))) OR (from_node_id = ? AND to_node_id = ?)")
            .bind(target).bind(owner).bind(&owner_wallet).bind(&owner_wallet).bind(owner).bind(target)
            .execute(&mut *transaction).await?;
        sqlx::query("DELETE FROM social_invites WHERE (recipient = ? AND sender = ?) OR (recipient = ? AND sender = ?)")
            .bind(owner).bind(target).bind(target).bind(owner).execute(&mut *transaction).await?;
        if let (Some(a), Some(b)) = (owner_wallet.as_deref(), target_wallet.as_deref()) {
            let (lo, hi) = ordered(a, b);
            sqlx::query("DELETE FROM wallet_friendships WHERE wallet_lo = ? AND wallet_hi = ?")
                .bind(lo)
                .bind(hi)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        info!("[Friends] {} blocked {}", owner, target);
        Ok(())
    }

    pub async fn unblock(&self, owner: &str, target: &str) -> Result<()> {
        sqlx::query("DELETE FROM blocks WHERE blocker_node_id = ? AND blocked_node_id = ?")
            .bind(owner)
            .bind(target)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn blocked_players(&self, owner: &str) -> Result<Vec<BlockedPlayer>> {
        let rows: Vec<(String, Option<String>, String, String)> = sqlx::query_as(
            "SELECT blocked_node_id, blocked_pubkey, blocked_display, created_at FROM blocks WHERE blocker_node_id = ? ORDER BY created_at DESC",
        )
        .bind(owner)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(node, pubkey, display, created)| BlockedPlayer {
                blocked_node_id: node,
                blocked_pubkey: pubkey,
                blocked_display: display,
                created_at: created.parse().unwrap_or_else(|_| Utc::now()),
            })
            .collect())
    }

    pub async fn save_invite(
        &self,
        owner: &str,
        invite: &super::routes::LobbyInvite,
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("DELETE FROM social_invites WHERE expires_at <= ?")
            .bind(Utc::now().timestamp())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO social_invites (id, recipient, sender, game_id, payload, expires_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(recipient, sender, game_id) DO NOTHING")
            .bind(&invite.id).bind(owner).bind(&invite.from_node_id).bind(&invite.game_id).bind(serde_json::to_string(invite)?)
            .bind((Utc::now() + chrono::Duration::minutes(15)).timestamp()).execute(&mut *transaction).await?;
        sqlx::query("DELETE FROM social_invites WHERE recipient = ? AND rowid NOT IN (SELECT rowid FROM social_invites WHERE recipient = ? ORDER BY rowid DESC LIMIT 50)")
            .bind(owner).bind(owner).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn invites(&self, owner: &str) -> Result<Vec<super::routes::LobbyInvite>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT payload FROM social_invites WHERE recipient = ? AND expires_at > ? ORDER BY rowid DESC LIMIT 50")
            .bind(owner).bind(Utc::now().timestamp()).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|(payload,)| serde_json::from_str(&payload).map_err(Into::into))
            .collect()
    }

    pub async fn dismiss_invite(&self, owner: &str, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM social_invites WHERE recipient = ? AND id = ?")
            .bind(owner)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn decode_requests(rows: Vec<RequestRow>) -> Vec<FriendRequest> {
    rows.into_iter()
        .map(
            |(id, from_node, from_pk, from_disp, to_node, to_pk, msg, status, created, to_disp)| {
                FriendRequest {
                    id,
                    from_node_id: from_node,
                    from_pubkey: from_pk,
                    from_display: from_disp,
                    to_node_id: to_node,
                    to_pubkey: to_pk,
                    to_display: to_disp,
                    message: msg,
                    status: match status.as_str() {
                        "accepted" => RequestStatus::Accepted,
                        "rejected" => RequestStatus::Rejected,
                        _ => RequestStatus::Pending,
                    },
                    created_at: created.parse().unwrap_or_else(|_| Utc::now()),
                    responded_at: None,
                }
            },
        )
        .collect()
}
