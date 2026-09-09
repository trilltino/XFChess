use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tracing::{info, warn};

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

pub struct KycInput<'a> {
    pub wallet_pubkey: &'a str,
    pub country: &'a str,
    pub full_name: &'a str,
    pub dob: &'a str,
    pub residence: &'a str,
    pub tax_id_raw: &'a str,
    pub data_source: &'a str,
}

fn blind_hash(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.trim().as_bytes());
    format!("{:x}", h.finalize())
}

#[derive(Clone)]
pub struct VaultStore {
    pool: SqlitePool,
    session_pool: SqlitePool,
}

impl VaultStore {
    pub fn new(pool: SqlitePool, session_pool: SqlitePool) -> Self {
        Self { pool, session_pool }
    }

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

    pub async fn get_kyc(&self, wallet_pubkey: &str) -> Option<KycRecord> {
        sqlx::query_as::<_, KycRecord>(
            "SELECT * FROM kyc_records WHERE wallet_pubkey = ?1 AND deleted_at IS NULL",
        )
        .bind(wallet_pubkey)
        .fetch_one(&self.pool)
        .await
        .ok()
    }

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

    pub async fn write_audit(&self, pubkey: &str, action: &str) {
        let now = chrono::Utc::now().timestamp();
        if let Err(e) =
            sqlx::query("INSERT INTO audit_log (pubkey, action, timestamp) VALUES (?1, ?2, ?3)")
                .bind(pubkey)
                .bind(action)
                .bind(now)
                .execute(&self.pool)
                .await
        {
            warn!(
                "[vault] audit log write failed for {}/{}: {}",
                pubkey, action, e
            );
        }
    }

    // ── CACF compliance persistence ────────────────────────────────────────────

    pub async fn save_cacf(
        &self,
        wallet: &str,
        country: &str,
        status: &str,
        kyc_completed: bool,
        details_json: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO cacf_compliance (wallet, country, status, kyc_completed, details_json, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(wallet, country) DO UPDATE SET
                status        = excluded.status,
                kyc_completed = excluded.kyc_completed,
                details_json  = excluded.details_json,
                updated_at    = excluded.updated_at
            "#,
        )
        .bind(wallet)
        .bind(country)
        .bind(status)
        .bind(kyc_completed as i32)
        .bind(details_json)
        .bind(now)
        .execute(&self.session_pool)
        .await?;
        Ok(())
    }

    pub async fn load_cacf_status(&self, wallet: &str, country: &str) -> Option<String> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT status FROM cacf_compliance WHERE wallet = ?1 AND country = ?2")
                .bind(wallet)
                .bind(country)
                .fetch_optional(&self.session_pool)
                .await
                .ok()
                .flatten();
        row.map(|(s,)| s)
    }

    pub async fn cacf_can_wager(&self, wallet: &str, country: &str) -> bool {
        match country {
            "GB" | "BR" | "DE" | "CA" => {
                match self.load_cacf_status(wallet, country).await.as_deref() {
                    Some("fully_compliant") | Some("partially_compliant") => true,
                    _ => false,
                }
            }
            _ => true,
        }
    }
}
