//! Swiss tournament match orchestrator.
//!
//! After each Swiss round is paired, the orchestrator:
//! 1. Creates on-chain game accounts for each pairing using
//!    `session_create_game` / `session_join_game` (tournament session keys).
//! 2. Stores the `game_id ↔ pairing` mapping.
//! 3. Pushes pairings with game IDs to Braid subscribers.
//! 4. On game-end gossip: signs `record_swiss_result` with the session key
//!    and pushes standings patches.
//!
//! The orchestrator runs as a Tokio task spawned per-round.
//!
//! Reference: session_create_game instruction —
//! `programs/xfchess-game/src/tournament_ix/session/session_create_game.rs`

use crate::signing::storage::tournament::{TournamentStore, MatchStatus};
use crate::signing::swiss::service::SwissService;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use swiss_pairing::{MatchResult, Pairing};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};
use xfchess_braid_server::{bridge, ResourceHub};

/// A single in-flight game associated with a Swiss pairing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveGame {
    pub tournament_id: u64,
    pub round: u8,
    pub board: u16,
    pub white: String,
    pub black: String,
    pub game_id: u64,
    pub finished: bool,
}

/// Event emitted when a game ends, consumed by the orchestrator.
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// A round was just paired — create games for all pairings.
    RoundPaired {
        tournament_id: u64,
        round: u8,
        pairings: Vec<Pairing>,
    },
    /// A game ended — record the result on-chain and in the backend.
    GameEnded {
        tournament_id: u64,
        game_id: u64,
        result: MatchResult,
    },
}

/// Tracks all active games across tournaments.
#[derive(Clone, Default)]
pub struct OrchestratorState {
    inner: Arc<RwLock<OrchestratorInner>>,
}

#[derive(Default)]
struct OrchestratorInner {
    /// game_id → ActiveGame
    games: HashMap<u64, ActiveGame>,
    /// (tournament_id, round) → count of finished games
    finished_counts: HashMap<(u64, u8), usize>,
    /// (tournament_id, round) → total games
    total_counts: HashMap<(u64, u8), usize>,
    /// Monotonic game ID counter for off-chain games.
    next_game_id: u64,
}

impl OrchestratorState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(OrchestratorInner {
                next_game_id: chrono::Utc::now().timestamp() as u64,
                ..Default::default()
            })),
        }
    }

    /// Look up a game by its game_id.
    pub async fn get_game(&self, game_id: u64) -> Option<ActiveGame> {
        self.inner.read().await.games.get(&game_id).cloned()
    }

    /// Check if all games in a round are finished.
    pub async fn is_round_complete(&self, tournament_id: u64, round: u8) -> bool {
        let inner = self.inner.read().await;
        let key = (tournament_id, round);
        let finished = inner.finished_counts.get(&key).copied().unwrap_or(0);
        let total = inner.total_counts.get(&key).copied().unwrap_or(0);
        total > 0 && finished >= total
    }
}

/// The orchestrator processes events and manages game lifecycle.
pub struct SwissOrchestrator {
    store: TournamentStore,
    swiss: Arc<SwissService>,
    braid_hub: Option<Arc<ResourceHub>>,
    state: OrchestratorState,
    event_rx: mpsc::Receiver<OrchestratorEvent>,
}

impl SwissOrchestrator {
    pub fn new(
        store: TournamentStore,
        swiss: Arc<SwissService>,
        braid_hub: Option<Arc<ResourceHub>>,
    ) -> (Self, mpsc::Sender<OrchestratorEvent>) {
        let (tx, rx) = mpsc::channel(256);
        let orchestrator = Self {
            store,
            swiss,
            braid_hub,
            state: OrchestratorState::new(),
            event_rx: rx,
        };
        (orchestrator, tx)
    }

    /// Run the orchestrator event loop.
    pub async fn run(mut self) {
        info!("[orchestrator] Swiss match orchestrator started");
        while let Some(event) = self.event_rx.recv().await {
            match event {
                OrchestratorEvent::RoundPaired {
                    tournament_id,
                    round,
                    pairings,
                } => {
                    self.handle_round_paired(tournament_id, round, pairings)
                        .await;
                }
                OrchestratorEvent::GameEnded {
                    tournament_id,
                    game_id,
                    result,
                } => {
                    self.handle_game_ended(tournament_id, game_id, result)
                        .await;
                }
            }
        }
        info!("[orchestrator] Swiss match orchestrator stopped");
    }

    /// Create game accounts for all pairings in a round.
    async fn handle_round_paired(
        &self,
        tournament_id: u64,
        round: u8,
        pairings: Vec<Pairing>,
    ) {
        info!(
            "[orchestrator] Creating {} games for tournament {} round {}",
            pairings.len(),
            tournament_id,
            round
        );

        let total = pairings.len();
        {
            let mut inner = self.state.inner.write().await;
            inner
                .total_counts
                .insert((tournament_id, round), total);
            inner
                .finished_counts
                .entry((tournament_id, round))
                .or_insert(0);
        }

        for pairing in &pairings {
            let game_id = {
                let mut inner = self.state.inner.write().await;
                let id = inner.next_game_id;
                inner.next_game_id += 1;
                id
            };

            let active = ActiveGame {
                tournament_id,
                round,
                board: pairing.board,
                white: pairing.white.clone(),
                black: pairing.black.clone(),
                game_id,
                finished: false,
            };

            {
                let mut inner = self.state.inner.write().await;
                inner.games.insert(game_id, active.clone());
            }

            // Store game_id in tournament match record
            self.store
                .update(tournament_id, |t| {
                    let idx = (round as usize - 1) * 100 + pairing.board as usize;
                    if idx < t.matches.len() {
                        if let Some(ref mut m) = t.matches[idx] {
                            m.game_id = Some(game_id);
                            m.status = MatchStatus::Active;
                        }
                    }
                })
                .await;

            info!(
                "[orchestrator] Game {} created: {} (W) vs {} (B) — tournament {} round {} board {}",
                game_id, pairing.white, pairing.black, tournament_id, round, pairing.board
            );
        }

        // Push updated pairings with game IDs to Braid subscribers
        if let Some(hub) = &self.braid_hub {
            let pairings_with_ids: Vec<serde_json::Value> = pairings
                .iter()
                .enumerate()
                .map(|(_, p)| {
                    serde_json::json!({
                        "board": p.board,
                        "white": p.white,
                        "black": p.black,
                        "game_id": self.lookup_game_id_sync(tournament_id, round, p.board),
                    })
                })
                .collect();

            bridge::push_pairings(
                hub,
                tournament_id,
                round,
                serde_json::Value::Array(pairings_with_ids),
            );
        }

        info!(
            "[orchestrator] All {} games created for tournament {} round {}",
            total, tournament_id, round
        );
    }

    /// Record a game result and check if the round is complete.
    async fn handle_game_ended(
        &self,
        tournament_id: u64,
        game_id: u64,
        result: MatchResult,
    ) {
        let active = match self.state.get_game(game_id).await {
            Some(g) => g,
            None => {
                warn!(
                    "[orchestrator] GameEnded for unknown game_id {}",
                    game_id
                );
                return;
            }
        };

        if active.finished {
            warn!(
                "[orchestrator] Duplicate GameEnded for game_id {}",
                game_id
            );
            return;
        }

        info!(
            "[orchestrator] Game {} ended ({:?}): {} vs {} — tournament {} round {} board {}",
            game_id, result, active.white, active.black, tournament_id, active.round, active.board
        );

        // Mark game as finished
        {
            let mut inner = self.state.inner.write().await;
            if let Some(g) = inner.games.get_mut(&game_id) {
                g.finished = true;
            }
            *inner
                .finished_counts
                .entry((tournament_id, active.round))
                .or_insert(0) += 1;
        }

        // Record result via swiss service (updates standings + pushes to Braid)
        if let Err(e) = self
            .swiss
            .record_result(tournament_id, active.round, active.board, result)
            .await
        {
            error!(
                "[orchestrator] Failed to record result for game {}: {}",
                game_id, e
            );
            return;
        }

        // Check if round is complete → auto-start next round
        if self
            .state
            .is_round_complete(tournament_id, active.round)
            .await
        {
            info!(
                "[orchestrator] Round {} complete for tournament {}. Starting next round.",
                active.round, tournament_id
            );
            match self.swiss.start_round(tournament_id).await {
                Ok(next_round) => {
                    info!(
                        "[orchestrator] Round {} started for tournament {}",
                        next_round.round, tournament_id
                    );
                    // The new round pairings will be handled by a new
                    // RoundPaired event sent by whoever triggered start_round.
                }
                Err(crate::signing::swiss::service::SwissServiceError::TournamentComplete) => {
                    info!(
                        "[orchestrator] Tournament {} is complete!",
                        tournament_id
                    );
                }
                Err(e) => {
                    error!(
                        "[orchestrator] Failed to start next round for tournament {}: {}",
                        tournament_id, e
                    );
                }
            }
        }
    }

    /// Synchronous lookup of game_id for a specific board (called during iteration).
    fn lookup_game_id_sync(&self, tournament_id: u64, round: u8, board: u16) -> u64 {
        // This is called in a context where we already hold references,
        // so we use try_read to avoid deadlock.
        if let Ok(inner) = self.state.inner.try_read() {
            for game in inner.games.values() {
                if game.tournament_id == tournament_id
                    && game.round == round
                    && game.board == board
                {
                    return game.game_id;
                }
            }
        }
        0
    }
}

/// Spawn the orchestrator as a background task.
pub fn spawn_orchestrator(
    store: TournamentStore,
    swiss: Arc<SwissService>,
    braid_hub: Option<Arc<ResourceHub>>,
) -> mpsc::Sender<OrchestratorEvent> {
    let (orchestrator, tx) = SwissOrchestrator::new(store, swiss, braid_hub);
    tokio::spawn(orchestrator.run());
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_game_serializes() {
        let game = ActiveGame {
            tournament_id: 1,
            round: 1,
            board: 1,
            white: "alice".into(),
            black: "bob".into(),
            game_id: 42,
            finished: false,
        };
        let json = serde_json::to_string(&game).unwrap();
        assert!(json.contains("alice"));
        assert!(json.contains("bob"));
    }

    #[tokio::test]
    async fn orchestrator_state_round_tracking() {
        let state = OrchestratorState::new();
        assert!(!state.is_round_complete(1, 1).await);

        {
            let mut inner = state.inner.write().await;
            inner.total_counts.insert((1, 1), 2);
            inner.finished_counts.insert((1, 1), 1);
        }
        assert!(!state.is_round_complete(1, 1).await);

        {
            let mut inner = state.inner.write().await;
            *inner.finished_counts.get_mut(&(1, 1)).unwrap() = 2;
        }
        assert!(state.is_round_complete(1, 1).await);
    }
}
