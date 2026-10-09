//! Durable cancellation evidence shared by owner actions and scheduled expiry.
use anyhow::{anyhow, ensure, Context, Result};
use borsh::BorshDeserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
use solana_sdk::{pubkey::Pubkey, signature::{Keypair, Signature, Signer}, transaction::Transaction};
use sqlx::SqlitePool;
use std::{str::FromStr, sync::Arc};
use super::{solana::{cancel_tournament_ix, make_rpc, transaction_fetch_config}, storage::tournament::{TournamentStatus, TournamentStore, TournamentTransaction}};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TournamentOperation {
    pub id: String,
    pub tournament_id: String,
    pub action: String,
    pub actor: String,
    pub reason: String,
    pub status: String,
    pub signature: Option<String>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn init(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("CREATE TABLE IF NOT EXISTS tournament_operations (
        id TEXT PRIMARY KEY, tournament_id TEXT NOT NULL, action TEXT NOT NULL,
        actor TEXT NOT NULL, reason TEXT NOT NULL, status TEXT NOT NULL,
        signature TEXT, last_error TEXT, lease_until INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)")
        .execute(pool).await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS archived_tournaments (
        tournament_id INTEGER PRIMARY KEY, archived_at INTEGER NOT NULL)")
        .execute(pool).await?;
    Ok(())
}

pub async fn list(pool: &SqlitePool, tournament_id: u64) -> Result<Vec<TournamentOperation>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM tournament_operations WHERE tournament_id = ? ORDER BY created_at DESC")
        .bind(tournament_id.to_string()).fetch_all(pool).await
}

async fn get(pool: &SqlitePool, id: &str) -> Result<TournamentOperation> {
    Ok(sqlx::query_as("SELECT * FROM tournament_operations WHERE id = ?")
        .bind(id).fetch_one(pool).await?)
}

#[derive(BorshDeserialize)]
struct TournamentPrefix {
    id: u64,
    authority: [u8; 32],
    _name: String,
    _entry_fee: u64,
    _platform_fee: u64,
    _prize_pool: u64,
    capacity: u16,
    _player_count: u16,
    registered: u16,
    status: u8,
}

#[derive(Clone)]
pub struct CancellationService {
    pub store: TournamentStore,
    pub rpc_url: String,
    pub program_id: Pubkey,
    pub authority: Arc<Keypair>,
    pub host_treasury: Pubkey,
}

impl CancellationService {
    pub async fn cancel(&self, tournament_id: u64, actor: &str, reason: &str) -> Result<TournamentOperation> {
        ensure!(!reason.trim().is_empty(), "Cancellation requires a reason");
        let id = format!("cancel_tournament:{tournament_id}");
        let pool = self.store.pool();
        sqlx::query("INSERT OR IGNORE INTO tournament_operations
            (id,tournament_id,action,actor,reason,status,created_at,updated_at)
            VALUES (?,?,'cancel_tournament',?,?,'requested',unixepoch(),unixepoch())")
            .bind(&id).bind(tournament_id.to_string()).bind(actor).bind(reason)
            .execute(pool).await?;
        let operation = get(pool, &id).await?;
        if matches!(operation.status.as_str(), "resolved" | "failed" | "needs_admin_review") {
            return Ok(operation);
        }
        let claimed = sqlx::query("UPDATE tournament_operations SET lease_until=unixepoch()+180
            WHERE id=? AND lease_until < unixepoch()")
            .bind(&id).execute(pool).await?.rows_affected();
        if claimed == 0 { return get(pool, &id).await; }
        let result = self.execute(tournament_id, &operation).await;
        match result {
            Ok(status) => {
                sqlx::query("UPDATE tournament_operations SET status=?,last_error=NULL,lease_until=0,updated_at=unixepoch() WHERE id=?")
                    .bind(status).bind(&id).execute(pool).await?;
            }
            Err(error) => {
                // A signed transaction may already have landed, even when send
                // returned an error. Keep its signature for chain reconciliation.
                tracing::warn!(event="tournament_cancel_pending", tournament_id, operation=%id, "Cancellation requires reconciliation");
                sqlx::query("UPDATE tournament_operations SET status='pending_reconciliation',last_error=?,lease_until=0,updated_at=unixepoch() WHERE id=?")
                    .bind(error.to_string()).bind(&id).execute(pool).await?;
            }
        }
        get(pool, &id).await
    }

    async fn execute(&self, tournament_id: u64, operation: &TournamentOperation) -> Result<&'static str> {
        let record = self.store.get(tournament_id).await.context("Tournament record unavailable")?;
        if let Some(signature) = &operation.signature {
            let sig = Signature::from_str(signature)?;
            let service = self.clone();
            let chain_result = tokio::task::spawn_blocking(move || -> Result<Option<bool>> {
                let rpc = make_rpc(&service.rpc_url);
                let statuses = rpc.get_signature_statuses_with_history(&[sig])?;
                let Some(status) = statuses.value.first().and_then(Option::as_ref) else { return Ok(None) };
                if status.err.is_some() { return Ok(Some(false)) }
                // Fetch at confirmed commitment; processed status alone is insufficient.
                let tx = rpc.get_transaction_with_config(&sig, transaction_fetch_config())?;
                ensure!(tx.transaction.meta.as_ref().is_some_and(|m| m.err.is_none()), "Missing successful transaction metadata");
                let chain = service.read_chain(tournament_id)?;
                ensure!(chain.status == 4, "Cancellation landed but tournament is not Cancelled");
                Ok(Some(true))
            }).await??;
            return match chain_result {
                Some(true) => {
                    self.store.update_checked(tournament_id, |t| { t.status = TournamentStatus::Cancelled; t.prize_pool = 0; }).await?;
                    ensure!(self.store.record_transaction(TournamentTransaction {
                        tournament_id, signature: signature.clone(), operation: "cancel_tournament".into(), status: "confirmed".into(),
                        retry_count: 0, last_error: None, next_retry_at: None, created_at: chrono::Utc::now().timestamp(),
                    }).await, "Could not persist refund evidence");
                    Ok("resolved")
                }
                Some(false) => Ok("failed"),
                None => Ok("pending_reconciliation"),
            };
        }
        let service = self.clone();
        let tx = tokio::task::spawn_blocking(move || -> Result<Transaction> {
            let chain = service.read_chain(tournament_id)?;
            ensure!(chain.status <= 1, "Tournament is terminal on-chain; review its existing evidence");
            ensure!(chain.registered as usize == record.players.len() && chain.capacity == record.max_players,
                "Stored roster differs from chain; reconcile registration before cancelling");
            ensure!(service.host_treasury == service.authority.pubkey(), "Host treasury signer unavailable");
            let players = record.players.iter().map(|p| Pubkey::from_str(p)).collect::<std::result::Result<Vec<_>, _>>()?;
            let rpc = make_rpc(&service.rpc_url);
            let ix = cancel_tournament_ix(&service.program_id, tournament_id, record.max_players,
                &service.authority.pubkey(), &service.host_treasury, &players);
            let tx = Transaction::new_signed_with_payer(&[ix], Some(&service.authority.pubkey()),
                &[service.authority.as_ref()], rpc.get_latest_blockhash()?);
            ensure!(bincode::serialized_size(&tx)? <= 1232, "Cancellation exceeds legacy transaction capacity; batch refund support is required");
            let simulation = rpc.simulate_transaction(&tx)?;
            ensure!(simulation.value.err.is_none(), "Cancellation simulation rejected: {:?}", simulation.value.err);
            Ok(tx)
        }).await??;
        let signature = tx.signatures[0].to_string();
        sqlx::query("UPDATE tournament_operations SET signature=?,status='submitted',updated_at=unixepoch() WHERE id=?")
            .bind(&signature).bind(&operation.id).execute(self.store.pool()).await?;
        let rpc_url = self.rpc_url.clone();
        tokio::task::spawn_blocking(move || make_rpc(&rpc_url).send_transaction(&tx)).await??;
        Ok("pending_reconciliation")
    }

    fn read_chain(&self, tournament_id: u64) -> Result<TournamentPrefix> {
        let pda = Pubkey::find_program_address(&[b"tournament", &tournament_id.to_le_bytes()], &self.program_id).0;
        let account = make_rpc(&self.rpc_url).get_account(&pda)?;
        ensure!(account.owner == self.program_id, "Tournament account has wrong owner");
        let discriminator = Sha256::digest(b"account:Tournament");
        ensure!(account.data.get(..8) == Some(&discriminator[..8]), "Invalid tournament discriminator");
        let prefix = TournamentPrefix::deserialize(&mut &account.data[8..])?;
        ensure!(prefix.id == tournament_id && prefix.authority == self.authority.pubkey().to_bytes(), "Wrong tournament identity or authority");
        Ok(prefix)
    }

    pub async fn reconcile_pending(&self) -> Result<()> {
        let ids: Vec<String> = sqlx::query_scalar("SELECT tournament_id FROM tournament_operations
            WHERE action='cancel_tournament' AND status IN ('requested','submitted','pending_reconciliation')
            AND lease_until < unixepoch() ORDER BY updated_at LIMIT 20")
            .fetch_all(self.store.pool()).await?;
        for id in ids {
            let id = id.parse().map_err(|_| anyhow!("Invalid operation tournament ID"))?;
            self.cancel(id, "reconciler", "Resume persisted cancellation").await?;
        }
        Ok(())
    }
}
