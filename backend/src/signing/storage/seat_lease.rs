//! A lease is keyed by (game_id, wallet). The newest device claim increments
//! epoch and makes previous devices view-only. A seat without a lease allows
//! legacy clients to write.

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

/// Bound opaque client device IDs to prevent arbitrary payload storage.
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
    ///
    /// `move_signer` replaces the seat's move-signing key, so a superseded
    /// device's signatures stop authorising relayed moves.
    pub async fn claim(
        &self,
        game_id: &str,
        wallet: &str,
        device_id: &str,
        move_signer: Option<&str>,
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
            "INSERT INTO game_seat_leases (game_id, wallet, device_id, epoch, updated_at, move_signer) \
             VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(game_id, wallet) DO UPDATE SET \
             device_id = excluded.device_id, epoch = excluded.epoch, \
             updated_at = excluded.updated_at, move_signer = excluded.move_signer",
        )
        .bind(game_id)
        .bind(wallet)
        .bind(device_id)
        .bind(epoch)
        .bind(now)
        .bind(move_signer)
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

    /// The move-signing key registered by the device holding `wallet`'s seat.
    pub async fn move_signer(
        &self,
        game_id: &str,
        wallet: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT move_signer FROM game_seat_leases WHERE game_id = ? AND wallet = ?",
        )
        .bind(game_id)
        .bind(wallet)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|(signer,)| signer))
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
        crate::infrastructure::migrate_session_db(&pool)
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

        let a = s.claim("7", "W", "device-aaaa", None, 1).await.unwrap();
        assert_eq!(a.epoch, 1);
        assert_eq!(
            s.check("7", "W", Some("device-aaaa")).await.unwrap(),
            SeatCheck::Holder
        );

        let b = s.claim("7", "W", "device-bbbb", None, 2).await.unwrap();
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
        s.claim("7", "W", "device-aaaa", None, 1).await.unwrap();
        assert_eq!(
            s.claim("7", "W", "device-aaaa", None, 2)
                .await
                .unwrap()
                .epoch,
            1
        );
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

    #[tokio::test]
    async fn the_newest_claim_owns_the_move_signer() {
        let s = store().await;
        assert_eq!(s.move_signer("7", "W").await.unwrap(), None);
        s.claim("7", "W", "device-aaaa", Some("KeyA"), 1)
            .await
            .unwrap();
        assert_eq!(
            s.move_signer("7", "W").await.unwrap().as_deref(),
            Some("KeyA")
        );
        // A takeover replaces the key, and a device without one clears it.
        s.claim("7", "W", "device-bbbb", Some("KeyB"), 2)
            .await
            .unwrap();
        assert_eq!(
            s.move_signer("7", "W").await.unwrap().as_deref(),
            Some("KeyB")
        );
        s.claim("7", "W", "device-cccc", None, 3).await.unwrap();
        assert_eq!(s.move_signer("7", "W").await.unwrap(), None);
        assert_eq!(s.move_signer("7", "B").await.unwrap(), None);
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
