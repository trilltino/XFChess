use crate::multiplayer::systems::fen_ply;
use bevy::prelude::*; // Events are in prelude
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum GameStateStatus {
    #[default]
    Synced,
    Pending,
    Committing,
    OutOfSync,
}

#[derive(Debug, Clone)]
pub struct PendingBatch {
    pub moves: Vec<String>,
    pub next_fens: Vec<String>,
    pub start_turn: u16,
    pub since: Instant,
}

#[derive(Resource)]
pub struct EphemeralRollupManager {
    // Committed baseline (from chain)
    pub committed_fen: String,
    pub committed_turn: u16,

    // Pending batch (ephemeral)
    pub pending_batch: Option<PendingBatch>,

    // Status
    pub status: GameStateStatus,

    // Configuration
    pub max_batch_size: usize,
    pub flush_interval: Duration,
    pub game_id: u64,
    pub session_keys: Option<(Pubkey, Pubkey)>, // (white_session_key, black_session_key)
    pub is_creator: bool,
    pub used_global_session: bool,
}

impl Default for EphemeralRollupManager {
    fn default() -> Self {
        Self::new(
            0,
            false,
            String::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        )
    }
}

impl EphemeralRollupManager {
    pub fn new(game_id: u64, is_creator: bool, initial_fen: String) -> Self {
        Self {
            committed_fen: initial_fen,
            committed_turn: 0,
            pending_batch: None,
            status: GameStateStatus::Synced,
            max_batch_size: 10,
            flush_interval: Duration::from_secs(10),
            game_id,
            session_keys: None,
            is_creator,
            used_global_session: false,
        }
    }

    pub fn default_for_game(game_id: u64) -> Self {
        Self::new(
            game_id,
            false,
            String::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        )
    }

    /// Point the manager at `game_id`. A different game always starts from a
    /// fresh baseline so the previous game's position, batch or out-of-sync
    /// flag can never gate or leak into the new one.
    pub fn assign_game(&mut self, game_id: u64) {
        if self.game_id != game_id {
            *self = Self::default_for_game(game_id);
        }
    }

    pub fn add_local_move(&mut self, move_uci: String, next_fen: String) {
        if self.status == GameStateStatus::OutOfSync {
            warn!("Attempting to add move to out-of-sync game");
            return;
        }

        match &mut self.pending_batch {
            Some(batch) => {
                batch.moves.push(move_uci);
                batch.next_fens.push(next_fen);

                if batch.moves.len() >= self.max_batch_size {
                    self.status = GameStateStatus::Pending;
                }
            }
            None => {
                self.pending_batch = Some(PendingBatch {
                    moves: vec![move_uci],
                    next_fens: vec![next_fen],
                    start_turn: self.committed_turn,
                    since: Instant::now(),
                });
                self.status = GameStateStatus::Pending;
            }
        }

        // Check if we need to flush
        if self.should_flush() {
            self.status = GameStateStatus::Pending;
        }
    }

    pub fn add_remote_move(&mut self, move_uci: String, next_fen: String) {
        // Validate expected turn here if needed
        self.add_local_move(move_uci, next_fen);
    }

    pub fn should_flush(&self) -> bool {
        match &self.pending_batch {
            Some(batch) => {
                batch.moves.len() >= self.max_batch_size
                    || batch.since.elapsed() >= self.flush_interval
            }
            None => false,
        }
    }

    pub fn prepare_batch_for_commit(&mut self) -> Option<(Vec<String>, Vec<String>)> {
        if self.status != GameStateStatus::Pending {
            return None;
        }

        match self.pending_batch.take() {
            Some(batch) => {
                self.status = GameStateStatus::Committing;
                Some((batch.moves, batch.next_fens))
            }
            None => None,
        }
    }

    pub fn batch_commit_success(&mut self, final_fen: String, move_count: usize) {
        self.committed_fen = final_fen;
        self.committed_turn = self.committed_turn.saturating_add(move_count as u16);
        self.status = GameStateStatus::Synced;
    }

    pub fn batch_commit_failed(&mut self, moves: Vec<String>, next_fens: Vec<String>) {
        // Restore the failed batch
        self.pending_batch = Some(PendingBatch {
            moves,
            next_fens,
            start_turn: self.committed_turn,
            since: Instant::now(),
        });
        self.status = GameStateStatus::Pending;
    }

    pub fn force_flush(&mut self) -> Option<(Vec<String>, Vec<String>)> {
        if let Some(batch) = &mut self.pending_batch {
            if !batch.moves.is_empty() {
                self.status = GameStateStatus::Pending;
                return self.prepare_batch_for_commit();
            }
        }
        None
    }

    pub fn reset(&mut self) {
        self.pending_batch = None;
        self.status = GameStateStatus::Synced;
    }

    pub fn set_session_keys(&mut self, white_session_key: Pubkey, black_session_key: Pubkey) {
        self.session_keys = Some((white_session_key, black_session_key));
    }

    /// Apply a committed baseline reported by a peer (`Committed`,
    /// `ResyncResponse`) or rebuilt from the move log (`SnapshotReceived`).
    ///
    /// None of those sources is the Game PDA, so they may only move the
    /// baseline forward. Ordering uses the FEN's own ply (side to move +
    /// fullmove number), not `committed_turn`: a stale position is ignored,
    /// and a different position at the same ply is a conflict that marks the
    /// game out of sync instead of overwriting ours. Returns whether the
    /// baseline was updated.
    pub fn accept_peer_baseline(&mut self, game_id: u64, fen: &str, turn: u16) -> bool {
        if game_id != self.game_id {
            return false;
        }
        let (Some(incoming), Some(current)) = (fen_ply(fen), fen_ply(&self.committed_fen)) else {
            warn!(
                "[ROLLUP] Ignoring malformed baseline FEN for game {}",
                game_id
            );
            return false;
        };
        if incoming < current {
            warn!(
                "[ROLLUP] Ignoring stale baseline for game {} (ply {} < committed {})",
                game_id, incoming, current
            );
            return false;
        }
        if incoming == current && fen != self.committed_fen {
            warn!(
                "[ROLLUP] Conflicting baseline for game {} at ply {}; marking out of sync",
                game_id, incoming
            );
            self.status = GameStateStatus::OutOfSync;
            return false;
        }
        self.committed_fen = fen.to_string();
        self.committed_turn = self.committed_turn.max(turn);
        if self.pending_batch.is_none() {
            self.status = GameStateStatus::Synced;
        }
        true
    }

    /// Apply one move recovered from the Braid log. A replayed or reordered
    /// resync whose position is not newer than the baseline is ignored.
    pub fn accept_resynced_move(&mut self, game_id: u64, next_fen: &str) -> bool {
        if game_id != self.game_id {
            return false;
        }
        match (fen_ply(next_fen), fen_ply(&self.committed_fen)) {
            (Some(incoming), Some(current)) if incoming > current => {
                self.committed_fen = next_fen.to_string();
                self.committed_turn = self.committed_turn.saturating_add(1);
                true
            }
            _ => false,
        }
    }
}

#[derive(Event, Message, Debug, Clone)]
pub enum RollupEvent {
    BatchReady {
        game_id: u64,
        moves: Vec<String>,
        next_fens: Vec<String>,
    },
    GameEndBatch {
        game_id: u64,
        moves: Vec<String>,
        next_fens: Vec<String>,
    },
    BatchCommitted {
        game_id: u64,
        new_fen: String,
        new_turn: u16,
    },
    BatchFailed {
        game_id: u64,
        moves: Vec<String>,
        next_fens: Vec<String>,
    },
    NeedResync {
        game_id: u64,
    },
    ResyncedMove {
        game_id: u64,
        move_uci: String,
        next_fen: String,
        move_number: u32,
    },
    SnapshotReceived {
        game_id: u64,
        fen: String,
        move_payloads: Vec<braid_chess::MovePayload>,
        head_version: String,
    },
}

pub struct EphemeralRollupPlugin;

impl Plugin for EphemeralRollupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EphemeralRollupManager>()
            .add_message::<RollupEvent>()
            .add_systems(Update, handle_rollup_events);
        // Periodic auto-flush used to be driven from here too
        // (`check_for_auto_flush`, since removed), racing
        // `bridge::process_batch_commit_requests` — both watched the same
        // `should_flush()`/`prepare_batch_for_commit()` state and whichever
        // system's turn came first that frame won `pending_batch.take()`.
        // This system's path sent the batch over the slow negotiated
        // `BatchPropose`/`BatchAccept` P2P round-trip
        // (`bridge::handle_rollup_to_network_events`'s `RollupEvent::BatchReady`
        // arm); if the game ended before the peer's accept came back, the
        // batch just sat in `bridge.pending_batches`, forever unsubmitted —
        // reproduced live: a game's first move (`f2f3`) never reached
        // `record_move` at all, and every move after it then failed replay
        // validation against a backend history that still didn't include it.
        // `process_batch_commit_requests` submits directly via the VPS with
        // no round-trip (the same path the game-end batch already used
        // successfully), so it's now the sole periodic-flush path.
    }
}

fn handle_rollup_events(
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    mut rollup_events: MessageReader<RollupEvent>,
) {
    for event in rollup_events.read() {
        match event {
            RollupEvent::BatchCommitted {
                game_id,
                new_fen,
                new_turn,
            } if *game_id == rollup_manager.game_id => {
                if rollup_manager.accept_peer_baseline(*game_id, new_fen, *new_turn) {
                    info!("Batch committed successfully for game {}", game_id);
                }
            }
            RollupEvent::ResyncedMove {
                game_id,
                move_uci,
                next_fen,
                ..
            } if *game_id == rollup_manager.game_id => {
                // A missed move arrived via Braid reconnection recovery; the
                // board itself is applied by the game-logic layer. Only a
                // strictly newer position may advance the baseline.
                if rollup_manager.accept_resynced_move(*game_id, next_fen) {
                    info!(
                        "[ROLLUP] ResyncedMove {} applied, turn now {}",
                        move_uci, rollup_manager.committed_turn
                    );
                }
            }
            RollupEvent::SnapshotReceived {
                game_id,
                fen,
                move_payloads,
                head_version,
            } if *game_id == rollup_manager.game_id => {
                let turn = move_payloads.len().min(u16::MAX as usize) as u16;
                if rollup_manager.accept_peer_baseline(*game_id, fen, turn) {
                    info!(
                        "[ROLLUP] SnapshotReceived: {} moves, head {}, fen set",
                        move_payloads.len(),
                        head_version
                    );
                }
            }
            RollupEvent::BatchFailed {
                game_id,
                moves,
                next_fens,
            } if *game_id == rollup_manager.game_id => {
                rollup_manager.batch_commit_failed(moves.clone(), next_fens.clone());
                info!(
                    "Batch commit failed for game {}, restored to pending",
                    game_id
                );
            }
            RollupEvent::NeedResync { game_id } if *game_id == rollup_manager.game_id => {
                rollup_manager.status = GameStateStatus::OutOfSync;
                warn!("Game {} marked as out of sync, requires resync", game_id);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const E4: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    const D4: &str = "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq d3 0 1";
    const E4E5: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2";

    fn manager() -> EphemeralRollupManager {
        EphemeralRollupManager::new(7, true, START.to_string())
    }

    #[test]
    fn peer_baseline_advances_but_never_rewinds() {
        let mut m = manager();
        assert!(m.accept_peer_baseline(7, E4E5, 2));
        assert_eq!(m.committed_fen, E4E5);
        // A delayed `Committed`/`ResyncResponse` for an older position.
        assert!(!m.accept_peer_baseline(7, E4, 1));
        assert!(!m.accept_peer_baseline(7, START, 0));
        assert_eq!(m.committed_fen, E4E5);
        assert_eq!(m.committed_turn, 2);
    }

    #[test]
    fn conflicting_baseline_at_same_ply_marks_out_of_sync() {
        let mut m = manager();
        assert!(m.accept_peer_baseline(7, E4, 1));
        assert!(!m.accept_peer_baseline(7, D4, 1));
        assert_eq!(m.committed_fen, E4);
        assert_eq!(m.status, GameStateStatus::OutOfSync);
    }

    #[test]
    fn baseline_for_another_game_or_malformed_fen_is_ignored() {
        let mut m = manager();
        assert!(!m.accept_peer_baseline(8, E4, 1));
        assert!(!m.accept_peer_baseline(7, "not a fen", 9));
        assert_eq!(m.committed_fen, START);
        assert_eq!(m.committed_turn, 0);
    }

    #[test]
    fn assigning_a_new_game_resets_the_previous_baseline() {
        let mut m = manager();
        assert!(m.accept_peer_baseline(7, E4E5, 2));
        m.status = GameStateStatus::OutOfSync;
        m.assign_game(7);
        assert_eq!(m.committed_fen, E4E5, "same game keeps its baseline");
        m.assign_game(8);
        assert_eq!(m.game_id, 8);
        assert_eq!(m.committed_fen, START);
        assert_eq!(m.committed_turn, 0);
        assert_eq!(m.status, GameStateStatus::Synced);
        // The new game's first position is accepted, not blocked by game 7's.
        assert!(m.accept_peer_baseline(8, E4, 1));
    }

    #[test]
    fn duplicate_or_stale_resynced_move_does_not_advance_turn() {
        let mut m = manager();
        assert!(m.accept_resynced_move(7, E4));
        assert!(!m.accept_resynced_move(7, E4));
        assert!(!m.accept_resynced_move(7, START));
        assert!(m.accept_resynced_move(7, E4E5));
        assert_eq!(m.committed_turn, 2);
        assert!(!m.accept_resynced_move(8, E4E5));
    }
}
