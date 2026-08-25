//! Persisted ledger of this wallet's own wagered on-chain games + recovery scan.
//!
//! Safety net against stranded wagers: the wager escrow PDA only releases its
//! SOL through the on-chain `cancel_game` instruction (or settlement /
//! expiry-withdrawal). Any lobby that disappears without one — app crash
//! mid-create, dismissed wallet popup, a Cancel button that only cleared
//! local UI state — leaves the wager locked in escrow with no trace in the
//! UI. This module remembers every wagered game this wallet created or joined
//! (a small JSON ledger under the config dir) so the lobby can resurface
//! still-recoverable ones as one-click refund entries.

use std::path::PathBuf;

use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;

use crate::solana::instructions::{GAME_SEED, PROGRAM_ID};

/// Cap so a long-tail ledger can never turn the scan into an unbounded RPC
/// storm — entries beyond this simply stop being tracked.
const MAX_LEDGER_ENTRIES: usize = 64;
/// How many most-recent entries each scan checks on-chain.
const SCAN_BATCH: usize = 20;

/// `GameStatus` Borsh discriminants (programs/xfchess-game/src/state/game.rs)
/// where escrow may still be recoverable via `cancel_game`:
/// 1 = WaitingForOpponent (creator cancels the open lobby),
/// 2 = Active (cancellable at zero moves, or after a 24h stall).
const STATUS_WAITING_FOR_OPPONENT: u8 = 1;
const STATUS_ACTIVE: u8 = 2;

/// Anchor layout offsets into the `Game` account (8-byte discriminator first):
/// game_id(8) white(32) black(32) => status byte at 80; `wager_amount` is
/// pinned at +212 by the program's `wager_amount_offset_is_212` test => 220.
const STATUS_OFFSET: usize = 8 + 8 + 32 + 32;
const WAGER_OFFSET: usize = 8 + 212;

/// A ledgered game that still holds this wallet's escrow on-chain.
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

/// Remember a wagered game this wallet just created or joined (most-recent
/// first, deduped, capped). Called only for non-zero wagers — free games have
/// nothing to recover.
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

/// Drop a game from the ledger — after a successful refund, or when the user
/// explicitly dismisses a dead entry.
pub fn forget(game_id: u64) {
    let mut ids = load();
    let before = ids.len();
    ids.retain(|&id| id != game_id);
    if ids.len() != before {
        save(&ids);
    }
}

/// Check recent ledgered games on-chain and keep those that still hold this
/// wallet's escrow and look cancellable right now. Entries whose account is
/// gone entirely are pruned from the ledger; finished/settled/expired/cancelled
/// games are skipped (nothing left to reclaim) but stay ledgered so a future
/// re-record doesn't churn the file.
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

/// Run [`scan_reclaimable`] off the render thread; result arrives via `tx`.
pub fn spawn_scan(wallet: Pubkey, rpc_url: String, tx: oneshot::Sender<Vec<ReclaimableWager>>) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let _ = tx.send(scan_reclaimable(&wallet, &rpc_url));
        })
        .detach();
}
