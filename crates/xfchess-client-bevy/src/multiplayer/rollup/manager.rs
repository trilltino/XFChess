use crate::game::systems::board_snapshot::verify_snapshot_history;
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

    pub pending_batch: Option<PendingBatch>,

    pub status: GameStateStatus,

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

    /// Select a game, resetting position, batches, and synchronization flags when the ID changes.
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

    /// Apply a peer or log baseline only forward by FEN ply. Ignore stale positions;
    /// mark conflicting positions at the same ply out of sync. Return whether updated.
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
            .add_systems(
                Update,
                handle_rollup_events.in_set(crate::core::schedule::SessionSet::Rollup),
            );
        // process_batch_commit_requests owns periodic flushing; a second flush path
        // could consume the batch before direct VPS submission.
    }
}

fn handle_rollup_events(
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    mut rollup_events: MessageReader<RollupEvent>,
    mut board: crate::game::systems::board_snapshot::RecoveryBoard,
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
                let verified = next_fen == &rollup_manager.committed_fen
                    || xfchess_rules::Game::from_fen(&rollup_manager.committed_fen)
                        .ok()
                        .is_some_and(|mut position| {
                            position.play_uci(move_uci).is_ok() && position.fen() == *next_fen
                        });
                if verified
                    && (next_fen == &rollup_manager.committed_fen
                        || rollup_manager.accept_resynced_move(*game_id, next_fen))
                {
                    // Spectators consume recovered plies through their existing
                    // move dispatcher, including its delay and application ACKs.
                    if !board.is_spectator() {
                        board.queue(*game_id, next_fen);
                    }
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
                let valid = verify_snapshot_history(
                    &rollup_manager.committed_fen,
                    fen,
                    move_payloads,
                    head_version,
                );
                let turn = move_payloads.len().min(u16::MAX as usize) as u16;
                if valid && rollup_manager.accept_peer_baseline(*game_id, fen, turn) {
                    board.queue(*game_id, fen);
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

    fn snapshot(ucis: &[&str]) -> Result<(String, Vec<braid_chess::MovePayload>, String), String> {
        let mut position = xfchess_rules::Game::default();
        let mut moves = Vec::new();
        for (index, uci) in ucis.iter().enumerate() {
            position.play_uci(uci)?;
            // The publisher numbers each player's own lane independently.
            moves.push(braid_chess::MovePayload::from_uci(
                *uci,
                position.fen(),
                index as u32 / 2 + 1,
                "player",
            ));
        }
        let head = moves
            .last()
            .map(|payload| braid_chess::version_hash(&payload.fen_after, payload.move_number))
            .unwrap_or_else(|| "0".into());
        Ok((position.fen(), moves, head))
    }

    fn recovery_app() -> App {
        use crate::game::resources::*;
        let mut app = App::new();
        app.insert_resource(State::new(crate::core::GameState::InGame));
        app.insert_resource(crate::core::GameMode::OnlineMultiplayer);
        app.insert_resource(
            crate::multiplayer::network::online_game_session::OnlineGameSession {
                active: true,
                game_id: "7".into(),
                ..Default::default()
            },
        );
        app.init_resource::<crate::engine::board_state::ChessEngine>()
            .init_resource::<CurrentTurn>()
            .init_resource::<PendingTurnAdvance>()
            .init_resource::<Selection>()
            .init_resource::<PendingPromotion>()
            .init_resource::<crate::motion::BoardEpoch>()
            .init_resource::<premove::Premove>();
        app.insert_resource(crate::game::systems::board_snapshot::PendingBoardSnapshot(
            Some(START.into()),
        ));
        app.add_message::<crate::game::events::MoveMadeEvent>();
        app.add_plugins(EphemeralRollupPlugin);
        app.insert_resource(manager());
        crate::core::schedule::configure(&mut app);
        app.add_systems(
            Update,
            crate::game::systems::board_snapshot::apply_board_snapshot
                .in_set(crate::core::schedule::GameSystems::Execution),
        );
        app.update();
        app
    }

    #[test]
    fn accepted_snapshot_reconstructs_the_board_and_retires_motion_and_input() -> Result<(), String>
    {
        use crate::rendering::pieces::Piece;
        use bevy::ecs::system::Command;
        let mut app = recovery_app();
        let old = app
            .world_mut()
            .query::<(Entity, &Piece)>()
            .iter(app.world())
            .find(|(_, piece)| (piece.x, piece.y) == (4, 1))
            .map(|(entity, _)| entity)
            .ok_or("missing pawn")?;
        crate::motion::StartPieceMotion::gameplay(old, (4, 1), (4, 3)).apply(app.world_mut());
        let epoch = app.world().resource::<crate::motion::BoardEpoch>().0;
        app.world_mut()
            .resource_mut::<crate::game::resources::premove::Premove>()
            .click((4, 1), true, epoch);
        let (fen, moves, head) = snapshot(&["e2e4", "e7e5"])?;
        app.world_mut()
            .write_message(RollupEvent::SnapshotReceived {
                game_id: 7,
                fen: fen.clone(),
                move_payloads: moves,
                head_version: head,
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::engine::board_state::ChessEngine>()
                .current_fen(),
            fen
        );
        assert_eq!(
            app.world()
                .resource::<crate::game::resources::CurrentTurn>()
                .move_number,
            2
        );
        assert!(app.world().get_entity(old).is_err());
        assert_eq!(
            app.world_mut()
                .query::<&crate::motion::PieceMotion>()
                .iter(app.world())
                .count(),
            0
        );
        assert!(!app
            .world()
            .resource::<crate::game::resources::premove::Premove>()
            .is_active());
        assert!(app
            .world()
            .resource::<Messages<crate::game::events::MoveMadeEvent>>()
            .is_empty());
        Ok(())
    }

    #[test]
    fn trusted_rollup_baseline_repairs_a_same_ply_gameplay_conflict() -> Result<(), String> {
        let mut app = recovery_app();
        app.world_mut()
            .resource_mut::<EphemeralRollupManager>()
            .accept_peer_baseline(7, E4, 1);
        app.world_mut()
            .resource_mut::<crate::engine::board_state::ChessEngine>()
            .set_from_fen(D4)?;
        let (fen, moves, head) = snapshot(&["e2e4"])?;
        app.world_mut()
            .write_message(RollupEvent::SnapshotReceived {
                game_id: 7,
                fen: fen.clone(),
                move_payloads: moves,
                head_version: head,
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::engine::board_state::ChessEngine>()
                .current_fen(),
            fen
        );
        Ok(())
    }

    #[test]
    fn snapshot_requires_legal_history_matching_the_anchor_and_head() -> Result<(), String> {
        let (fen, moves, head) = snapshot(&["e2e4", "e7e5"])?;
        assert!(verify_snapshot_history(E4, &fen, &moves, &head));
        assert!(!verify_snapshot_history(D4, &fen, &moves, &head));
        assert!(!verify_snapshot_history(START, &fen, &moves, "wrong-head"));
        assert!(!verify_snapshot_history(START, E4, &moves, &head));
        let mut illegal = moves.clone();
        illegal[0].uci = "e2e4q".into();
        assert!(!verify_snapshot_history(START, &fen, &illegal, &head));
        Ok(())
    }

    #[test]
    fn snapshot_cannot_rewind_gameplay_or_bypass_spectator_delay() -> Result<(), String> {
        for delayed in [false, true] {
            let mut app = recovery_app();
            let expected = if delayed {
                app.insert_resource(crate::core::GameMode::Spectator);
                let mut session = crate::multiplayer::spectator::SpectatorSession::default();
                session.game_id = Some("7".into());
                session.delayed = true;
                session.delay_checked = true;
                app.insert_resource(session);
                START.to_string()
            } else {
                let (fen, _, _) = snapshot(&["e2e4", "e7e5", "g1f3"])?;
                app.world_mut()
                    .resource_mut::<crate::engine::board_state::ChessEngine>()
                    .set_from_fen(&fen)?;
                fen
            };
            let (fen, moves, head) = snapshot(&["e2e4", "e7e5"])?;
            app.world_mut()
                .write_message(RollupEvent::SnapshotReceived {
                    game_id: 7,
                    fen,
                    move_payloads: moves,
                    head_version: head,
                });
            app.update();
            assert_eq!(
                app.world()
                    .resource::<crate::engine::board_state::ChessEngine>()
                    .current_fen(),
                expected
            );
        }
        Ok(())
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
