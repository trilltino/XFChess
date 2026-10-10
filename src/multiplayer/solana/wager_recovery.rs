//! Find recoverable wagers from program-owned Game PDAs and the local ledger.
//! The ledger also covers delegated accounts. Report RPC failures as incomplete
//! scans, not an empty recovery set.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anchor_lang::{AccountDeserialize, Discriminator};
use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{
    RpcAccountInfoConfig, RpcProgramAccountsConfig, UiAccountEncoding,
};
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;
use xfchess_game::state::{Game, GameStatus};

use crate::solana::instructions::{GAME_SEED, PROGRAM_ID, WAGER_ESCROW_SEED};

/// Mirrors the program's `cancel` rule for an active game with moves
/// (`game_ix/cancel.rs`: 24 hours since `updated_at`).
pub const ABANDONED_CANCEL_AFTER_SECS: i64 = 3600 * 24;

/// RPC memcmp offsets skip the discriminator and game ID;
/// fully decode matched accounts with Game before using them.
const WHITE_FILTER_OFFSET: usize = 8 + 8;
const BLACK_FILTER_OFFSET: usize = WHITE_FILTER_OFFSET + 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Creator's game never got an opponent; `cancel_game` refunds the stake.
    CancelUnjoined,
    /// Opponent joined but nobody moved; either player may cancel and both
    /// stakes are refunded.
    CancelBeforeFirstMove,
    /// Game with moves has been idle past the program's abandonment window;
    /// either player may cancel and both stakes are refunded.
    CancelAbandoned,
    /// Cancelled game with escrow funds remaining. cancel_game idempotently refunds the remainder.
    CompleteCancellation,
    /// Delegated accounts must be undelegated or recovered before base-layer refunds.
    /// Display them without client cancellation actions.
    UndelegationRequired,
}

#[derive(Debug, Clone)]
pub struct ReclaimableWager {
    pub game_id: u64,
    pub wager_lamports: u64,
    pub action: RecoveryAction,
    /// Seconds until an active game becomes cancellable as abandoned, when
    /// it is still in play. `None` for every actionable entry.
    pub cancellable_in_secs: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct WagerScan {
    pub reclaimable: Vec<ReclaimableWager>,
    /// Wagered games still in play (not yet cancellable) or awaiting ER
    /// undelegation; informational only.
    pub in_play: Vec<ReclaimableWager>,
    /// False when an RPC call failed, so absence of a game proves nothing.
    pub complete: bool,
    pub errors: Vec<String>,
}

/// Decide what, if anything, `wallet` can still do about `game`'s escrow.
/// `escrow_lamports` is the current balance of the game's escrow PDA.
pub fn classify(
    game: &Game,
    wallet: &Pubkey,
    escrow_lamports: u64,
    owned_by_program: bool,
    now: i64,
) -> Option<ReclaimableWager> {
    let is_white = game.white == *wallet;
    let is_black = game.black == *wallet && game.black != Pubkey::default();
    if (!is_white && !is_black) || game.wager_amount == 0 {
        return None;
    }
    let entry = |action, cancellable_in_secs| ReclaimableWager {
        game_id: game.game_id,
        wager_lamports: game.wager_amount,
        action,
        cancellable_in_secs,
    };
    if !owned_by_program || game.is_delegated {
        return matches!(
            game.status,
            GameStatus::WaitingForOpponent | GameStatus::Active | GameStatus::Finished
        )
        .then(|| entry(RecoveryAction::UndelegationRequired, None));
    }
    match game.status {
        GameStatus::WaitingForOpponent if is_white => {
            Some(entry(RecoveryAction::CancelUnjoined, None))
        }
        GameStatus::Active if game.move_count == 0 => {
            Some(entry(RecoveryAction::CancelBeforeFirstMove, None))
        }
        GameStatus::Active => {
            let idle = now.saturating_sub(game.updated_at);
            if idle > ABANDONED_CANCEL_AFTER_SECS {
                Some(entry(RecoveryAction::CancelAbandoned, None))
            } else {
                Some(entry(
                    RecoveryAction::CancelAbandoned,
                    Some(ABANDONED_CANCEL_AFTER_SECS + 1 - idle),
                ))
            }
        }
        GameStatus::Cancelled if escrow_lamports >= game.wager_amount => {
            Some(entry(RecoveryAction::CompleteCancellation, None))
        }
        _ => None,
    }
}

/// Decode a Game account with the program's own type (checks discriminator
/// and the full Borsh layout instead of reading fixed byte offsets).
pub fn decode_game(data: &[u8]) -> Option<Game> {
    let mut slice = data;
    Game::try_deserialize(&mut slice).ok()
}

pub fn escrow_pda(program_id: &Pubkey, game_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], program_id).0
}

pub fn game_pda(program_id: &Pubkey, game_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0
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

fn load() -> Option<Vec<u64>> {
    match std::fs::read_to_string(ledger_path()) {
        Ok(json) => match serde_json::from_str(&json) {
            Ok(ids) => Some(ids),
            Err(e) => {
                warn!("[WAGER-RECOVERY] Ledger is unreadable; preserving the file: {e}");
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(Vec::new()),
        Err(e) => {
            warn!("[WAGER-RECOVERY] Ledger could not be read: {e}");
            None
        }
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
    let Some(mut ids) = load() else { return };
    ids.retain(|&id| id != game_id);
    ids.insert(0, game_id);
    save(&ids);
}

pub fn forget(game_id: u64) {
    let Some(mut ids) = load() else { return };
    let before = ids.len();
    ids.retain(|&id| id != game_id);
    if ids.len() != before {
        save(&ids);
    }
}

fn program_games_for(
    rpc: &RpcClient,
    program_id: &Pubkey,
    wallet: &Pubkey,
    offset: usize,
) -> Result<Vec<(Pubkey, Vec<u8>)>, String> {
    let config = RpcProgramAccountsConfig {
        filters: Some(vec![
            RpcFilterType::Memcmp(Memcmp::new_raw_bytes(0, Game::DISCRIMINATOR.to_vec())),
            RpcFilterType::Memcmp(Memcmp::new_raw_bytes(offset, wallet.to_bytes().to_vec())),
        ]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            ..Default::default()
        },
        ..Default::default()
    };
    rpc.get_program_ui_accounts_with_config(program_id, config)
        .map(|accounts| {
            accounts
                .into_iter()
                .filter_map(|(k, a)| a.data.decode().map(|data| (k, data)))
                .collect()
        })
        .map_err(|e| e.to_string())
}

fn scan(wallet: &Pubkey, rpc_url: &str) -> WagerScan {
    let mut out = WagerScan {
        complete: true,
        ..Default::default()
    };
    let program_id = match PROGRAM_ID.parse::<Pubkey>() {
        Ok(p) => p,
        Err(e) => {
            out.complete = false;
            out.errors.push(format!("bad program id: {e}"));
            return out;
        }
    };
    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // (game_id, decoded game, owned_by_program)
    let mut games: Vec<(Game, bool)> = Vec::new();
    let mut seen = BTreeSet::new();
    for offset in [WHITE_FILTER_OFFSET, BLACK_FILTER_OFFSET] {
        match program_games_for(&rpc, &program_id, wallet, offset) {
            Ok(accounts) => {
                for (_, data) in accounts {
                    if let Some(game) = decode_game(&data) {
                        if seen.insert(game.game_id) {
                            games.push((game, true));
                        }
                    }
                }
            }
            Err(e) => {
                warn!("[WAGER-RECOVERY] getProgramAccounts failed: {e}");
                out.complete = false;
                out.errors.push(format!("chain search failed: {e}"));
            }
        }
    }

    // Ledger IDs the program-owned scan did not return: delegated games, or
    // games missed because the scan above failed.
    match load() {
        Some(ids) => {
            let unseen: Vec<u64> = ids.into_iter().filter(|id| !seen.contains(id)).collect();
            for game_id in unseen {
                let pda = game_pda(&program_id, game_id);
                match rpc.get_account_with_commitment(&pda, CommitmentConfig::confirmed()) {
                    Ok(resp) => match resp.value {
                        Some(account) => match decode_game(&account.data) {
                            Some(game) => {
                                seen.insert(game_id);
                                games.push((game, account.owner == program_id));
                            }
                            None => out
                                .errors
                                .push(format!("game {game_id}: account could not be decoded")),
                        },
                        // Closed or never created: nothing on chain to refund
                        // from; keep the ledger entry for support reconciliation.
                        None => {}
                    },
                    Err(e) => {
                        out.complete = false;
                        out.errors
                            .push(format!("game {game_id}: lookup failed: {e}"));
                    }
                }
            }
        }
        None => out.errors.push("local wager ledger unreadable".to_string()),
    }

    for (game, owned_by_program) in games {
        let escrow = if game.status == GameStatus::Cancelled {
            match rpc.get_balance(&escrow_pda(&program_id, game.game_id)) {
                Ok(lamports) => lamports,
                Err(e) => {
                    out.complete = false;
                    out.errors
                        .push(format!("game {}: escrow lookup failed: {e}", game.game_id));
                    continue;
                }
            }
        } else {
            0
        };
        if let Some(entry) = classify(&game, wallet, escrow, owned_by_program, now) {
            // A delegated game is normally just being played on the ER; it is
            // listed for visibility but never offered as a client action.
            if entry.cancellable_in_secs.is_some()
                || entry.action == RecoveryAction::UndelegationRequired
            {
                out.in_play.push(entry);
            } else {
                out.reclaimable.push(entry);
            }
        }
    }
    out.reclaimable
        .sort_by_key(|w| std::cmp::Reverse(w.game_id));
    out
}

pub fn spawn_scan(wallet: Pubkey, rpc_url: String, tx: oneshot::Sender<WagerScan>) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let _ = tx.send(scan(&wallet, &rpc_url));
        })
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use xfchess_game::state::{GameResult, GameType, MatchType};

    fn game(status: GameStatus, white: Pubkey, black: Pubkey) -> Game {
        Game {
            game_id: 42,
            white,
            black,
            status,
            last_move_timestamp: 0,
            fees_advanced: 0,
            fee_payer: Pubkey::new_unique(),
            result: GameResult::None,
            board_state: [0; 68],
            move_count: 0,
            halfmove_clock: 0,
            turn: 1,
            created_at: 1_000,
            updated_at: 1_000,
            wager_amount: 5_000_000,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Rated,
            country_fee: 0,
            base_time_seconds: 600,
            increment_seconds: 0,
            bump: 255,
            is_delegated: false,
            tournament_id: None,
            nonce: 0,
            draw_offered_by: None,
        }
    }

    #[test]
    fn decodes_the_program_layout_including_a_winner_result() {
        // A `Winner` result shifts every later field by 32 bytes, which the
        // old fixed `8 + 212` wager offset silently misread.
        let w = Pubkey::new_unique();
        let mut g = game(GameStatus::Finished, w, Pubkey::new_unique());
        g.result = GameResult::Winner(w);
        let mut data = Vec::new();
        anchor_lang::AccountSerialize::try_serialize(&g, &mut data).unwrap();
        let decoded = decode_game(&data).expect("decodes");
        assert_eq!(decoded.wager_amount, 5_000_000);
        assert_eq!(decoded.result, GameResult::Winner(w));
        assert!(decode_game(&data[..40]).is_none());
        assert!(decode_game(&[0u8; 300]).is_none());
    }

    #[test]
    fn only_the_creator_can_reclaim_an_unjoined_game() {
        let w = Pubkey::new_unique();
        let g = game(GameStatus::WaitingForOpponent, w, Pubkey::default());
        let r = classify(&g, &w, 0, true, 2_000).unwrap();
        assert_eq!(r.action, RecoveryAction::CancelUnjoined);
        assert!(classify(&g, &Pubkey::default(), 0, true, 2_000).is_none());
        assert!(classify(&g, &Pubkey::new_unique(), 0, true, 2_000).is_none());
    }

    #[test]
    fn zero_move_active_game_is_reclaimable_by_either_player() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let g = game(GameStatus::Active, w, b);
        for who in [w, b] {
            let r = classify(&g, &who, 0, true, 1_001).unwrap();
            assert_eq!(r.action, RecoveryAction::CancelBeforeFirstMove);
            assert_eq!(r.cancellable_in_secs, None);
        }
    }

    #[test]
    fn active_game_with_moves_is_in_play_until_the_abandonment_window() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let mut g = game(GameStatus::Active, w, b);
        g.move_count = 3;
        let early = classify(&g, &w, 0, true, 1_000 + 60).unwrap();
        assert!(early.cancellable_in_secs.is_some());
        let late = classify(&g, &b, 0, true, 1_000 + ABANDONED_CANCEL_AFTER_SECS + 1).unwrap();
        assert_eq!(late.action, RecoveryAction::CancelAbandoned);
        assert_eq!(late.cancellable_in_secs, None);
    }

    #[test]
    fn cancelled_game_is_only_reclaimable_while_escrow_still_holds_a_stake() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let g = game(GameStatus::Cancelled, w, b);
        assert!(classify(&g, &w, 0, true, 2_000).is_none());
        assert!(classify(&g, &w, 4_999_999, true, 2_000).is_none());
        let r = classify(&g, &w, 5_000_000, true, 2_000).unwrap();
        assert_eq!(r.action, RecoveryAction::CompleteCancellation);
    }

    #[test]
    fn settled_finished_or_free_games_are_not_reclaimable() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        for status in [
            GameStatus::Settled,
            GameStatus::Finished,
            GameStatus::Expired,
        ] {
            assert!(classify(&game(status, w, b), &w, 10_000_000, true, 2_000).is_none());
        }
        let mut free = game(GameStatus::WaitingForOpponent, w, Pubkey::default());
        free.wager_amount = 0;
        assert!(classify(&free, &w, 0, true, 2_000).is_none());
    }

    #[test]
    fn delegated_game_requires_undelegation_not_a_client_cancel() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let mut g = game(GameStatus::Active, w, b);
        g.is_delegated = true;
        let r = classify(&g, &w, 0, true, 2_000).unwrap();
        assert_eq!(r.action, RecoveryAction::UndelegationRequired);
        let g = game(GameStatus::Active, w, b);
        let r = classify(&g, &b, 0, false, 2_000).unwrap();
        assert_eq!(r.action, RecoveryAction::UndelegationRequired);
    }
}
