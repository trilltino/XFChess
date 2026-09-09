use std::path::PathBuf;

use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;

use crate::solana::instructions::{GAME_SEED, PROGRAM_ID};

const MAX_LEDGER_ENTRIES: usize = 64;
const SCAN_BATCH: usize = 20;

const STATUS_WAITING_FOR_OPPONENT: u8 = 1;
const STATUS_ACTIVE: u8 = 2;

const STATUS_OFFSET: usize = 8 + 8 + 32 + 32;
const WAGER_OFFSET: usize = 8 + 212;

#[derive(Debug, Clone)]
pub struct ReclaimableWager {
    pub game_id: u64,
    pub wager_lamports: u64,
}

fn ledger_path() -> PathBuf {
    #[cfg(target_os = "android")]
    let base = crate::core::paths::internal_data_dir().unwrap_or_else(|| PathBuf::from("."));
    #[cfg(not(target_os = "android"))]
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess");
    base.join("wagered_games.json")
}

fn load() -> Vec<u64> {
    match std::fs::read_to_string(ledger_path()) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save(ids: &[u64]) {
    if let Some(dir) = ledger_path().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(ids) {
        Ok(json) => {
            if let Err(e) = std::fs::write(ledger_path(), json) {
                warn!("[WAGER-RECOVERY] Failed to save ledger: {e}");
            }
        }
        Err(e) => warn!("[WAGER-RECOVERY] Failed to serialize ledger: {e}"),
    }
}

pub fn record(game_id: u64) {
    if game_id == 0 {
        return;
    }
    let mut ids = load();
    ids.retain(|&id| id != game_id);
    ids.insert(0, game_id);
    ids.truncate(MAX_LEDGER_ENTRIES);
    save(&ids);
}

pub fn forget(game_id: u64) {
    let mut ids = load();
    let before = ids.len();
    ids.retain(|&id| id != game_id);
    if ids.len() != before {
        save(&ids);
    }
}

fn scan_reclaimable(wallet: &Pubkey, rpc_url: &str) -> Vec<ReclaimableWager> {
    let program_id = match PROGRAM_ID.parse::<Pubkey>() {
        Ok(p) => p,
        Err(e) => {
            warn!("[WAGER-RECOVERY] Bad PROGRAM_ID, skipping scan: {e}");
            return Vec::new();
        }
    };
    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    let mut out = Vec::new();
    for game_id in load().into_iter().take(SCAN_BATCH) {
        let game_pda =
            Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
        let data = match rpc.get_account_data(&game_pda) {
            Ok(d) => d,
            // Account never existed / fully gone — nothing to reclaim ever
            // again; prune so the ledger self-cleans.
            Err(e) => {
                let s = e.to_string();
                if s.contains("not found") || s.contains("AccountNotFound") {
                    forget(game_id);
                }
                continue;
            }
        };
        if data.len() <= STATUS_OFFSET {
            continue;
        }
        let status = data[STATUS_OFFSET];
        if status != STATUS_WAITING_FOR_OPPONENT && status != STATUS_ACTIVE {
            continue;
        }
        let wager_lamports = if data.len() >= WAGER_OFFSET + 8 {
            u64::from_le_bytes(data[WAGER_OFFSET..WAGER_OFFSET + 8].try_into().unwrap())
        } else {
            0
        };
        if wager_lamports == 0 {
            continue;
        }
        let is_participant = data
            .get(16..48)
            .map(|b| b == wallet.as_ref())
            .unwrap_or(false)
            || data
                .get(48..80)
                .map(|b| b == wallet.as_ref())
                .unwrap_or(false);
        if !is_participant {
            continue;
        }
        out.push(ReclaimableWager {
            game_id,
            wager_lamports,
        });
    }
    out
}

pub fn spawn_scan(wallet: Pubkey, rpc_url: String, tx: oneshot::Sender<Vec<ReclaimableWager>>) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let _ = tx.send(scan_reclaimable(&wallet, &rpc_url));
        })
        .detach();
}
