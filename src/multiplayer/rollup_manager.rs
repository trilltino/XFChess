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
}

impl Default for EphemeralRollupManager {
    fn default() -> Self {
        Self::new(
            0,
            String::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        )
    }
}

impl EphemeralRollupManager {
    pub fn new(game_id: u64, initial_fen: String) -> Self {
        Self {
            committed_fen: initial_fen,
            committed_turn: 0,
            pending_batch: None,
            status: GameStateStatus::Synced,
            max_batch_size: 10,
            flush_interval: Duration::from_secs(10),
            game_id,
            session_keys: None,
        }
    }

    pub fn default_for_game(game_id: u64) -> Self {
        Self::new(
            game_id,
            String::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        )
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

    pub fn batch_commit_success(&mut self, final_fen: String) {
        self.committed_fen = final_fen;
        self.committed_turn += self
            .pending_batch
            .as_ref()
            .map_or(0, |b| b.moves.len() as u16);
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
}

#[derive(Event, Message, Debug, Clone)]
pub enum RollupEvent {
    BatchReady {
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
}

pub struct EphemeralRollupPlugin;

impl Plugin for EphemeralRollupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EphemeralRollupManager>()
            .add_message::<RollupEvent>()
            .add_systems(Update, (handle_rollup_events, check_for_auto_flush));
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
                rollup_manager.committed_fen = new_fen.clone();
                rollup_manager.committed_turn = *new_turn;
                rollup_manager.status = GameStateStatus::Synced;
                info!("Batch committed successfully for game {}", game_id);
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

fn check_for_auto_flush(
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    mut rollup_events: MessageWriter<RollupEvent>,
) {
    if rollup_manager.status == GameStateStatus::Pending && rollup_manager.should_flush() {
        if let Some((moves, next_fens)) = rollup_manager.prepare_batch_for_commit() {
            rollup_events.write(RollupEvent::BatchReady {
                game_id: rollup_manager.game_id,
                moves,
                next_fens,
            });
        }
    }
}
