//! One playing device per player per game (migration 034).
//!
//! Policy: **the most recent device to claim a seat takes it over; every
//! earlier device becomes view-only.** A player who crashes and restarts, or
//! moves from desktop to phone, is never locked out of their own game — and a
//! forgotten second window can no longer submit moves that conflict with the
//! device actually being played on.
//!
//! A lease is keyed by `(game_id, wallet)`. Claiming bumps `epoch`; writes
//! carry the writer's `device_id` and are refused when another device holds
//! the seat. No lease means no client has opted in for that seat yet (older
//! clients), so writes stay allowed exactly as before.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeatLease {
    pub device_id: String,
    pub epoch: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatCheck {
    /// No device has claimed this seat; legacy behaviour applies.
    Unclaimed,
    /// The writer is the current holder.
    Holder,
    /// Another device took the seat over (or the writer sent no device id
    /// for a claimed seat). The write must be refused.
    Superseded(SeatLease),
}

#[derive(Clone)]
pub struct SeatLeaseStore {
    pool: sqlx::SqlitePool,
}

/// Device ids are client-generated opaque tokens; bound their shape so the
/// column can't be used to store arbitrary payloads.
pub fn valid_device_id(device_id: &str) -> bool {
    (8..=64).contains(&device_id.len())
        && device_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl SeatLeaseStore {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    /// Take the seat for `device_id`. Re-claiming by the current holder keeps
    /// the epoch (idempotent); a different device increments it.
    pub async fn claim(
        &self,
        game_id: &str,
        wallet: &str,
        device_id: &str,
        now: i64,
    ) -> Result<SeatLease, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let current: Option<(String, i64)> = sqlx::query_as(
            "SELECT device_id, epoch FROM game_seat_leases WHERE game_id = ? AND wallet = ?",
        )
        .bind(game_id)
        .bind(wallet)
        .fetch_optional(&mut *tx)
        .await?;
        let epoch = match &current {
            Some((holder, epoch)) if holder == device_id => *epoch,
            Some((_, epoch)) => epoch + 1,
            None => 1,
        };
        sqlx::query(
            "INSERT INTO game_seat_leases (game_id, wallet, device_id, epoch, updated_at) \
             VALUES (?, ?, ?, ?, ?) ON CONFLICT(game_id, wallet) DO UPDATE SET \
             device_id = excluded.device_id, epoch = excluded.epoch, \
             updated_at = excluded.updated_at",
        )
        .bind(game_id)
        .bind(wallet)
        .bind(device_id)
        .bind(epoch)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(SeatLease {
            device_id: device_id.to_string(),
            epoch,
        })
    }

    pub async fn get(&self, game_id: &str, wallet: &str) -> Result<Option<SeatLease>, sqlx::Error> {
        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT device_id, epoch FROM game_seat_leases WHERE game_id = ? AND wallet = ?",
        )
        .bind(game_id)
        .bind(wallet)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(device_id, epoch)| SeatLease { device_id, epoch }))
    }

    /// Whether `device_id` may write for `wallet`'s seat in `game_id`.
    pub async fn check(
        &self,
        game_id: &str,
        wallet: &str,
        device_id: Option<&str>,
    ) -> Result<SeatCheck, sqlx::Error> {
        Ok(match self.get(game_id, wallet).await? {
            None => SeatCheck::Unclaimed,
            Some(lease) if Some(lease.device_id.as_str()) == device_id => SeatCheck::Holder,
            Some(lease) => SeatCheck::Superseded(lease),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn store() -> SeatLeaseStore {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        // Same schema as migrations/034_game_seat_leases.sql.
        sqlx::query(
            "CREATE TABLE game_seat_leases (game_id TEXT NOT NULL, wallet TEXT NOT NULL, \
             device_id TEXT NOT NULL, epoch INTEGER NOT NULL, updated_at INTEGER NOT NULL, \
             PRIMARY KEY (game_id, wallet))",
        )
        .execute(&pool)
        .await
        .unwrap();
        SeatLeaseStore::new(pool)
    }

    #[tokio::test]
    async fn newest_device_takes_over_and_old_device_is_superseded() {
        let s = store().await;
        assert_eq!(
            s.check("7", "W", Some("device-aaaa")).await.unwrap(),
            SeatCheck::Unclaimed
        );

        let a = s.claim("7", "W", "device-aaaa", 1).await.unwrap();
        assert_eq!(a.epoch, 1);
        assert_eq!(
            s.check("7", "W", Some("device-aaaa")).await.unwrap(),
            SeatCheck::Holder
        );

        let b = s.claim("7", "W", "device-bbbb", 2).await.unwrap();
        assert_eq!(b.epoch, 2);
        assert_eq!(
            s.check("7", "W", Some("device-bbbb")).await.unwrap(),
            SeatCheck::Holder
        );
        assert_eq!(
            s.check("7", "W", Some("device-aaaa")).await.unwrap(),
            SeatCheck::Superseded(b.clone())
        );
        // A claimed seat refuses writes that carry no device id at all.
        assert_eq!(
            s.check("7", "W", None).await.unwrap(),
            SeatCheck::Superseded(b)
        );
    }

    #[tokio::test]
    async fn reclaim_by_holder_is_idempotent_and_seats_are_independent() {
        let s = store().await;
        s.claim("7", "W", "device-aaaa", 1).await.unwrap();
        assert_eq!(s.claim("7", "W", "device-aaaa", 2).await.unwrap().epoch, 1);
        // The opponent's seat and other games are untouched.
        assert_eq!(
            s.check("7", "B", Some("device-aaaa")).await.unwrap(),
            SeatCheck::Unclaimed
        );
        assert_eq!(
            s.check("8", "W", Some("device-aaaa")).await.unwrap(),
            SeatCheck::Unclaimed
        );
    }

    #[test]
    fn device_id_shape_is_bounded() {
        assert!(valid_device_id("0123456789abcdef"));
        assert!(valid_device_id("a1b2-c3d4_e5f6"));
        assert!(!valid_device_id("short"));
        assert!(!valid_device_id(&"x".repeat(65)));
        assert!(!valid_device_id("has space 12345"));
        assert!(!valid_device_id("quote'1234567"));
    }
}
