//! GDPR-compliant vault storage for KYC records and audit logs.
//!
//! All PII is stored in the dedicated vault SQLite database (separate from
//! the session/auth database). Tax IDs are stored only as SHA-256 blind
//! hashes — raw values never touch disk.
//!
//! GDPR right-to-erasure is supported via soft-delete (`deleted_at`) on
//! `kyc_records` and hard-nulling of PII fields.

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tracing::{info, warn};

/// Stored KYC record (read from DB).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KycRecord {
    pub id: i64,
    pub wallet_pubkey: String,
    pub country: String,
    pub full_name: String,
    pub dob: String,
    pub residence: String,
    pub tax_id_hash: String,
    pub data_source: String,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
}

/// KYC verification status for a wallet.
#[derive(Debug, Clone, PartialEq)]
pub enum KycStatus {
    None,
    Pending,
    Approved,
    Rejected,
}

impl KycStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            KycStatus::None => "none",
            KycStatus::Pending => "pending",
            KycStatus::Approved => "approved",
            KycStatus::Rejected => "rejected",
        }
    }
}

/// Input for a new KYC submission.
pub struct KycInput<'a> {
    pub wallet_pubkey: &'a str,
    pub country: &'a str,
    pub full_name: &'a str,
    pub dob: &'a str,
    pub residence: &'a str,
    /// Raw tax ID — hashed internally before storage; never persisted raw.
    pub tax_id_raw: &'a str,
    pub data_source: &'a str,
}

/// SHA-256 blind hash of a string (lowercase hex).
/// The raw value is never returned or stored.
fn blind_hash(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.trim().as_bytes());
    format!("{:x}", h.finalize())
}

/// SQLite-backed vault store for KYC records and audit logs.
#[derive(Clone)]
pub struct VaultStore {
    pool: SqlitePool,
}

impl VaultStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Inserts a new KYC record. Upserts on wallet_pubkey conflict.
    /// Tax ID is hashed before storage — raw value is dropped immediately.
    pub async fn insert_kyc(&self, input: KycInput<'_>) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        let hash = blind_hash(input.tax_id_raw);

        sqlx::query(
            r#"
            INSERT INTO kyc_records
                (wallet_pubkey, country, full_name, dob, residence, tax_id_hash, data_source, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(wallet_pubkey) DO UPDATE SET
                country      = excluded.country,
                full_name    = excluded.full_name,
                dob          = excluded.dob,
                residence    = excluded.residence,
                tax_id_hash  = excluded.tax_id_hash,
                data_source  = excluded.data_source,
                created_at   = excluded.created_at,
                deleted_at   = NULL
            "#,
        )
        .bind(input.wallet_pubkey)
        .bind(input.country)
        .bind(input.full_name)
        .bind(input.dob)
        .bind(input.residence)
        .bind(&hash)
        .bind(input.data_source)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.write_audit(input.wallet_pubkey, "kyc_submitted").await;
        info!("[vault] KYC record stored for {}", input.wallet_pubkey);
        Ok(())
    }

    /// Returns the active KYC record for a wallet, or None if erased/absent.
    pub async fn get_kyc(&self, wallet_pubkey: &str) -> Option<KycRecord> {
        sqlx::query_as::<_, KycRecord>(
            "SELECT * FROM kyc_records WHERE wallet_pubkey = ?1 AND deleted_at IS NULL",
        )
        .bind(wallet_pubkey)
        .fetch_one(&self.pool)
        .await
        .ok()
    }

    /// Returns true if an active (non-erased) KYC record exists.
    pub async fn has_kyc(&self, wallet_pubkey: &str) -> bool {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM kyc_records WHERE wallet_pubkey = ?1 AND deleted_at IS NULL",
        )
        .bind(wallet_pubkey)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0,));
        count > 0
    }

    /// GDPR right-to-erasure: soft-deletes the KYC record and nulls PII.
    /// The row is retained for audit trail with only the wallet_pubkey and timestamps.
    pub async fn erase_kyc(&self, wallet_pubkey: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE kyc_records SET
                full_name   = '[erased]',
                dob         = '[erased]',
                residence   = '[erased]',
                tax_id_hash = '[erased]',
                deleted_at  = ?1
            WHERE wallet_pubkey = ?2
            "#,
        )
        .bind(now)
        .bind(wallet_pubkey)
        .execute(&self.pool)
        .await?;

        self.write_audit(wallet_pubkey, "kyc_erased").await;
        info!("[vault] KYC PII erased for {}", wallet_pubkey);
        Ok(())
    }

    /// Logs a GDPR deletion request (right-to-erasure request from user).
    pub async fn log_deletion_request(
        &self,
        wallet_pubkey: &str,
        email: Option<&str>,
        reason: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO deletion_requests (wallet_pubkey, email, reason, requested_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(wallet_pubkey)
        .bind(email)
        .bind(reason)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.write_audit(wallet_pubkey, "deletion_requested").await;
        Ok(())
    }

    /// Marks a deletion request as completed.
    pub async fn complete_deletion_request(&self, wallet_pubkey: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "UPDATE deletion_requests SET completed_at = ?1 WHERE wallet_pubkey = ?2 AND completed_at IS NULL",
        )
        .bind(now)
        .bind(wallet_pubkey)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Appends an entry to the audit log. Best-effort — failures are logged.
    pub async fn write_audit(&self, pubkey: &str, action: &str) {
        let now = chrono::Utc::now().timestamp();
        if let Err(e) = sqlx::query(
            "INSERT INTO audit_log (pubkey, action, timestamp) VALUES (?1, ?2, ?3)",
        )
        .bind(pubkey)
        .bind(action)
        .bind(now)
        .execute(&self.pool)
        .await
        {
            warn!("[vault] audit log write failed for {}/{}: {}", pubkey, action, e);
        }
    }
}
