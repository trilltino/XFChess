//! Swiss tournament service — pairing, scoring, and live broadcast.
//!
//! [`SwissService`] owns the per-tournament Swiss lifecycle on top of the
//! persistent [`TournamentStore`]: initializing Swiss data, generating
//! pairings for each round, recording match results, and recomputing
//! standings with Buchholz/Sonneborn tiebreakers. When a round completes
//! it auto-starts the next one. Updates are optionally fanned out to
//! iroh gossip subscribers and Braid hub listeners so clients see live
//! pairings and standings without polling.
//!
//! On-chain and backend scoring conventions differ (integer points vs.
//! FIDE float); helpers `to_contract_points` / `from_contract_points`
//! convert between them.

use crate::signing::storage::tournament::{TournamentRecord, TournamentStatus, TournamentStore};
use crate::signing::tournament_gossip::TournamentGossipService;
use xfchess_braid_server::{bridge, ResourceHub};
use braid_iroh::protocol::{MatchResult as SwissMessageResult, SwissMessage, SwissPairing, SwissStandingsEntry};
// Note: bytes crate not available, using Vec<u8> instead
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use swiss_pairing::{
    calculate_standings, generate_pairings, Color, MatchResult, SwissPlayer, SwissRound,
    StandingsEntry,
};
use tracing::{info, warn};

/// Swiss-specific tournament data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwissData {
    /// Current round (1-indexed)
    pub current_round: u8,
    /// Total rounds in tournament
    pub total_rounds: u8,
    /// All completed and current rounds
    pub rounds: Vec<SwissRound>,
    /// Match results: (round, board, result)
    pub results: Vec<(u8, u16, MatchResult)>,
    /// Current standings
    pub standings: Vec<StandingsEntry>,
}

/// Swiss tournament service
#[derive(Clone)]
pub struct SwissService {
    store: TournamentStore,
    gossip: Option<Arc<TournamentGossipService>>,
    braid_hub: Option<Arc<ResourceHub>>,
}

impl SwissService {
    /// Create a new Swiss service
    pub fn new(store: TournamentStore) -> Self {
        Self {
            store,
            gossip: None,
            braid_hub: None,
        }
    }

    /// Attach the Braid resource hub so live updates stream to subscribers.
    pub fn set_braid_hub(&mut self, hub: Arc<ResourceHub>) {
        self.braid_hub = Some(hub);
    }

    /// Set the gossip service for broadcasting updates
    pub fn set_gossip(&mut self, gossip: Arc<TournamentGossipService>) {
        self.gossip = Some(gossip);
    }

    /// Initialize a Swiss tournament
    pub async fn initialize_swiss(
        &self,
        tournament_id: u64,
        rounds: u8,
    ) -> Result<(), SwissServiceError> {
        info!("Initializing Swiss tournament {} with {} rounds", tournament_id, rounds);

        self.store
            .update(tournament_id, |t| {
                t.format = crate::signing::storage::tournament::TournamentFormat::Swiss { rounds };
                t.swiss_data = Some(crate::signing::storage::tournament::SwissStorageData {
                    current_round: 0,
                    total_rounds: rounds,
                    rounds: Vec::new(),
                    results: Vec::new(),
                    standings: Vec::new(),
                });
                t.status = TournamentStatus::Active;
                t.started_at = Some(chrono::Utc::now().timestamp());
            })
            .await;

        Ok(())
    }

    /// Start the next round of a Swiss tournament
    pub async fn start_round(&self, tournament_id: u64) -> Result<SwissRound, SwissServiceError> {
        info!("Starting round for tournament {}", tournament_id);

        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let swiss_data = tournament
            .swiss_data
            .as_ref()
            .ok_or(SwissServiceError::NotSwissFormat)?;

        let next_round = swiss_data.current_round + 1;
        if next_round > swiss_data.total_rounds {
            return Err(SwissServiceError::TournamentComplete);
        }

        // Build player list with current scores
        let players = self.build_swiss_players(&tournament).await?;

        // Generate pairings
        let round = generate_pairings(next_round, &players, swiss_data.total_rounds)
            .map_err(|e| SwissServiceError::PairingError(e.to_string()))?;

        // Update tournament state
        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    sd.current_round = next_round;
                    sd.rounds.push(round.clone());
                }
                // status and started_at are already set by start_tournament().
            })
            .await;

        info!(
            "Round {} started for tournament {}: {} pairings, {} byes",
            next_round,
            tournament_id,
            round.pairings.len(),
            round.byes.len()
        );

        // Broadcast round started via gossip
        self.broadcast_round_started(tournament_id, next_round, &round).await;

        // Push pairings to Braid subscribers
        if let Some(hub) = &self.braid_hub {
            let pairings_json = serde_json::to_value(&round.pairings).unwrap_or_default();
            bridge::push_pairings(hub, tournament_id, next_round, pairings_json);
        }

        Ok(round)
    }

    /// Record a match result and update standings
    pub async fn record_result(
        &self,
        tournament_id: u64,
        round: u8,
        board: u16,
        result: MatchResult,
    ) -> Result<Vec<StandingsEntry>, SwissServiceError> {
        info!(
            "Recording result for tournament {} round {} board {:?}: {:?}",
            tournament_id, round, board, result
        );

        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let mut swiss_data = tournament
            .swiss_data
            .clone()
            .ok_or(SwissServiceError::NotSwissFormat)?;

        // Validate (round, board) before persisting to avoid corrupt state.
        let round_data = swiss_data
            .rounds
            .iter()
            .find(|r| r.round == round)
            .ok_or(SwissServiceError::InvalidRound(round))?;
        let _pairing = round_data
            .pairings
            .iter()
            .find(|p| p.board == board)
            .ok_or(SwissServiceError::InvalidBoard(board))?;

        swiss_data.results.push((round, board, result));

        // Rebuild player scores and calculate standings
        let players = self.build_swiss_players_with_results(&tournament, &swiss_data).await?;
        let standings = calculate_standings(&players, &swiss_data.rounds, &swiss_data.results);

        // Update stored standings
        swiss_data.standings = standings.clone();

        let is_last_round = swiss_data.current_round >= swiss_data.total_rounds;
        let is_round_complete = round_data.pairings.len()
            == swiss_data
                .results
                .iter()
                .filter(|(rnum, _, _)| *rnum == round)
                .count();

        self.store
            .update(tournament_id, |t| {
                t.swiss_data = Some(crate::signing::storage::tournament::SwissStorageData {
                    current_round: swiss_data.current_round,
                    total_rounds: swiss_data.total_rounds,
                    rounds: swiss_data.rounds.clone(),
                    results: swiss_data.results.clone(),
                    standings: standings.clone(),
                });

                // If last round, mark complete
                if round == swiss_data.total_rounds {
                    t.status = TournamentStatus::Completed;
                    t.completed_at = Some(chrono::Utc::now().timestamp());

                    // Set final placements for top 8
                    for (i, entry) in standings.iter().enumerate().take(8) {
                        match i {
                            0 => t.winner = Some(entry.player_id.clone()),
                            1 => t.second_place = Some(entry.player_id.clone()),
                            2 => t.third_place = Some(entry.player_id.clone()),
                            3 => t.fourth_place = Some(entry.player_id.clone()),
                            _ => {}
                        }
                    }
                }
            })
            .await;

        if is_round_complete && !is_last_round {
            let _ = self.start_round(tournament_id).await;
        }

        // Broadcast result and standings via gossip
        self.broadcast_result_recorded(tournament_id, round, board, &result).await;
        self.broadcast_standings_updated(tournament_id, &standings).await;

        // Push standings to Braid subscribers
        if let Some(hub) = &self.braid_hub {
            let standings_json = serde_json::to_value(&standings).unwrap_or_default();
            bridge::push_standings(hub, tournament_id, standings_json);
        }

        Ok(standings)
    }

    /// Broadcast round started message via gossip
    async fn broadcast_round_started(&self, tournament_id: u64, round: u8, swiss_round: &SwissRound) {
        let Some(gossip) = &self.gossip else { return };
        let Some(sender) = gossip.get_topic(tournament_id).await else {
            warn!("[swiss] No gossip topic for tournament {}", tournament_id);
            return;
        };

        let pairings: Vec<SwissPairing> = swiss_round
            .pairings
            .iter()
            .map(|p| SwissPairing {
                white: p.white.clone(),
                black: p.black.clone(),
                board: p.board,
            })
            .collect();

        let message = SwissMessage::RoundStarted {
            tournament_id,
            round,
            pairings,
        };

        let bytes = match serde_json::to_vec(&message) {
            Ok(b) => b,
            Err(e) => {
                warn!("[swiss] Failed to serialize RoundStarted: {}", e);
                return;
            }
        };

        if let Err(e) = sender.broadcast(bytes.into()).await {
            warn!("[swiss] Failed to broadcast RoundStarted: {}", e);
        } else {
            info!("[swiss] Broadcast RoundStarted for tournament {} round {}", tournament_id, round);
        }
    }

    /// Broadcast result recorded message via gossip
    async fn broadcast_result_recorded(
        &self,
        tournament_id: u64,
        round: u8,
        board: u16,
        result: &MatchResult,
    ) {
        let Some(gossip) = &self.gossip else { return };
        let Some(sender) = gossip.get_topic(tournament_id).await else {
            warn!("[swiss] No gossip topic for tournament {}", tournament_id);
            return;
        };

        let msg_result = match result {
            MatchResult::WhiteWin => SwissMessageResult::Win {
                winner: "white".to_string(),
            },
            MatchResult::BlackWin => SwissMessageResult::Win {
                winner: "black".to_string(),
            },
            MatchResult::Draw => SwissMessageResult::Draw,
            _ => {
                warn!("[swiss] Unknown match result variant");
                return;
            }
        };

        let message = SwissMessage::ResultRecorded {
            tournament_id,
            round,
            board,
            result: msg_result,
        };

        let bytes = match serde_json::to_vec(&message) {
            Ok(b) => b,
            Err(e) => {
                warn!("[swiss] Failed to serialize ResultRecorded: {}", e);
                return;
            }
        };

        if let Err(e) = sender.broadcast(bytes.into()).await {
            warn!("[swiss] Failed to broadcast ResultRecorded: {}", e);
        } else {
            info!("[swiss] Broadcast ResultRecorded for tournament {} round {} board {}", tournament_id, round, board);
        }
    }

    /// Broadcast standings updated message via gossip
    async fn broadcast_standings_updated(&self, tournament_id: u64, standings: &[StandingsEntry]) {
        let Some(gossip) = &self.gossip else { return };
        let Some(sender) = gossip.get_topic(tournament_id).await else {
            warn!("[swiss] No gossip topic for tournament {}", tournament_id);
            return;
        };

        let entries: Vec<SwissStandingsEntry> = standings
            .iter()
            .map(|s| SwissStandingsEntry {
                player_id: s.player_id.clone(),
                score: s.score,
                rank: s.rank,
            })
            .collect();

        let message = SwissMessage::StandingsUpdated {
            tournament_id,
            standings: entries,
        };

        let bytes = match serde_json::to_vec(&message) {
            Ok(b) => b,
            Err(e) => {
                warn!("[swiss] Failed to serialize StandingsUpdated: {}", e);
                return;
            }
        };

        if let Err(e) = sender.broadcast(bytes.into()).await {
            warn!("[swiss] Failed to broadcast StandingsUpdated: {}", e);
        } else {
            info!("[swiss] Broadcast StandingsUpdated for tournament {} ({} entries)", tournament_id, standings.len());
        }
    }

    /// Get current pairings for a round
    pub async fn get_pairings(
        &self,
        tournament_id: u64,
        round: u8,
    ) -> Result<Option<SwissRound>, SwissServiceError> {
        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let swiss_data = tournament
            .swiss_data
            .ok_or(SwissServiceError::NotSwissFormat)?;

        Ok(swiss_data.rounds.iter().find(|r| r.round == round).cloned())
    }

    /// Get current standings
    pub async fn get_standings(
        &self,
        tournament_id: u64,
    ) -> Result<Vec<StandingsEntry>, SwissServiceError> {
        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let swiss_data = tournament
            .swiss_data
            .ok_or(SwissServiceError::NotSwissFormat)?;

        Ok(swiss_data.standings)
    }

    /// Get current round number
    pub async fn get_current_round(&self, tournament_id: u64) -> Result<u8, SwissServiceError> {
        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let swiss_data = tournament
            .swiss_data
            .ok_or(SwissServiceError::NotSwissFormat)?;

        Ok(swiss_data.current_round)
    }

    /// Get the total configured rounds for the Swiss tournament.
    pub async fn get_total_rounds(&self, tournament_id: u64) -> Result<u8, SwissServiceError> {
        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let swiss_data = tournament
            .swiss_data
            .ok_or(SwissServiceError::NotSwissFormat)?;

        Ok(swiss_data.total_rounds)
    }

    /// Build SwissPlayer list from tournament data
    async fn build_swiss_players(
        &self,
        tournament: &TournamentRecord,
    ) -> Result<Vec<SwissPlayer>, SwissServiceError> {
        let swiss_data = tournament
            .swiss_data
            .as_ref()
            .ok_or(SwissServiceError::NotSwissFormat)?;

        self.build_swiss_players_with_results(tournament, swiss_data)
            .await
    }

    /// Build SwissPlayer list with scores from results
    async fn build_swiss_players_with_results(
        &self,
        tournament: &TournamentRecord,
        swiss_data: &crate::signing::storage::tournament::SwissStorageData,
    ) -> Result<Vec<SwissPlayer>, SwissServiceError> {
        let mut players: HashMap<String, SwissPlayer> = tournament
            .players
            .iter()
            .zip(tournament.player_elos.iter())
            .map(|(id, elo)| {
                (
                    id.clone(),
                    SwissPlayer {
                        id: id.clone(),
                        rating: *elo,
                        score: 0.0,
                        color_history: Vec::new(),
                        opponents: Vec::new(),
                        bye_count: 0,
                        float_status: swiss_pairing::FloatStatus::None,
                    },
                )
            })
            .collect();

        // Apply results to update scores
        for (round_num, board, result) in &swiss_data.results {
            let round = swiss_data
                .rounds
                .iter()
                .find(|r| r.round == *round_num)
                .ok_or_else(|| SwissServiceError::InvalidRound(*round_num))?;

            let pairing = round
                .pairings
                .iter()
                .find(|p| p.board == *board)
                .ok_or_else(|| SwissServiceError::InvalidBoard(*board))?;

            // Update white player
            if let Some(white) = players.get_mut(&pairing.white) {
                white.score += result.white_score();
                white.opponents.push(pairing.black.clone());
                white.color_history.push(Color::White);
            }

            // Update black player
            if let Some(black) = players.get_mut(&pairing.black) {
                black.score += result.black_score();
                black.opponents.push(pairing.white.clone());
                black.color_history.push(Color::Black);
            }
        }

        // Handle byes
        for round in &swiss_data.rounds {
            for bye_player_id in &round.byes {
                if let Some(player) = players.get_mut(bye_player_id) {
                    player.score += 1.0; // Bye = full point
                    player.bye_count += 1;
                }
            }
        }

        // Rebuild float_status from the most recent completed round so the
        // pairing engine can avoid floating the same player down twice.
        if let Some(last_round) = swiss_data.rounds.last() {
            for float_down_id in &last_round.float_downs {
                if let Some(player) = players.get_mut(float_down_id) {
                    player.float_status = swiss_pairing::FloatStatus::Down;
                }
            }
        }

        Ok(players.into_values().collect())
    }
}

// ── Scoring conversion (contract ↔ backend) ────────────────────────────────
//
// On-chain uses integer points (2/1/0), pairing engine uses FIDE float (1.0/0.5/0.0).
// See `SCORING.md` for the full mapping.

/// Convert a backend float score to on-chain integer points.
pub fn to_contract_points(score: f64) -> u8 {
    (score * 2.0).round() as u8
}

/// Convert an on-chain integer score to backend float.
pub fn from_contract_points(points: u8) -> f64 {
    points as f64 / 2.0
}

/// Errors that can occur in Swiss service
#[derive(Debug, thiserror::Error)]
pub enum SwissServiceError {
    #[error("Tournament not found")]
    TournamentNotFound,

    #[error("Tournament is not Swiss format")]
    NotSwissFormat,

    #[error("Tournament is complete")]
    TournamentComplete,

    #[error("Invalid round: {0}")]
    InvalidRound(u8),

    #[error("Invalid board: {0}")]
    InvalidBoard(u16),

    #[error("Pairing error: {0}")]
    PairingError(String),

    #[error("Player not found: {0}")]
    PlayerNotFound(String),

    #[error("Invalid result format")]
    InvalidResult,
}
