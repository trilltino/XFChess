use crate::signing::storage::tournament::{TournamentRecord, TournamentStatus, TournamentStore};
use xfchess_braid_server::{bridge, ResourceHub};
// Note: bytes crate not available, using Vec<u8> instead
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use swiss_pairing::{
    calculate_standings, generate_pairings, Color, ManualPairing, MatchResult, PairingConfig,
    StandingsEntry, SwissPlayer, SwissRound,
};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwissData {
    pub current_round: u8,
    pub total_rounds: u8,
    pub rounds: Vec<SwissRound>,
    pub results: Vec<(u8, u16, MatchResult)>,
    pub standings: Vec<StandingsEntry>,
    pub absent_players: Vec<String>,
    pub withdrawn_players: Vec<String>,
    pub forbidden_pairs: Vec<(String, String)>,
    pub manual_pairings_next_round: Vec<ManualPairing>,
}

#[derive(Clone)]
pub struct SwissService {
    store: TournamentStore,
    braid_hub: Option<Arc<ResourceHub>>,
}

impl SwissService {
    pub fn new(store: TournamentStore) -> Self {
        Self {
            store,
            braid_hub: None,
        }
    }

    pub fn set_braid_hub(&mut self, hub: Arc<ResourceHub>) {
        self.braid_hub = Some(hub);
    }

    pub async fn initialize_swiss(
        &self,
        tournament_id: u64,
        rounds: u8,
    ) -> Result<(), SwissServiceError> {
        info!(
            "Initializing Swiss tournament {} with {} rounds",
            tournament_id, rounds
        );

        self.store
            .update(tournament_id, |t| {
                t.format = crate::signing::storage::tournament::TournamentFormat::Swiss { rounds };
                t.swiss_data = Some(crate::signing::storage::tournament::SwissStorageData {
                    current_round: 0,
                    total_rounds: rounds,
                    rounds: Vec::new(),
                    results: Vec::new(),
                    standings: Vec::new(),
                    round_deadline_at: None,
                    absent_players: Vec::new(),
                    withdrawn_players: Vec::new(),
                    forbidden_pairs: Vec::new(),
                    manual_pairings_next_round: Vec::new(),
                });
                t.status = TournamentStatus::Active;
                t.started_at = Some(chrono::Utc::now().timestamp());
            })
            .await;

        Ok(())
    }

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

        // Build pairing config from stored forbidden pairs and manual overrides
        let config = PairingConfig {
            forbidden: swiss_data.forbidden_pairs.clone(),
            manual_overrides: swiss_data.manual_pairings_next_round.clone(),
        };

        // Generate pairings (absent/withdrawn flags already set on SwissPlayer)
        let round = generate_pairings(next_round, &players, swiss_data.total_rounds, &config)
            .map_err(|e| SwissServiceError::PairingError(e.to_string()))?;

        // Update tournament state — clear manual pairings after use
        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    sd.current_round = next_round;
                    sd.rounds.push(round.clone());
                    sd.manual_pairings_next_round.clear();
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

        // Publish the round's pairings. The orchestrator republishes this same
        // resource once it has created the games, adding a `game_id` per board;
        // that is a second version of one resource, not a second writer.
        if let Some(hub) = &self.braid_hub {
            let pairings_json = serde_json::to_value(&round.pairings).unwrap_or_default();
            bridge::push_pairings(hub, tournament_id, next_round, pairings_json);
        }

        Ok(round)
    }

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
        let players = self
            .build_swiss_players_with_results(&tournament, &swiss_data)
            .await?;
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
                    round_deadline_at: None,
                    absent_players: swiss_data.absent_players.clone(),
                    withdrawn_players: swiss_data.withdrawn_players.clone(),
                    forbidden_pairs: swiss_data.forbidden_pairs.clone(),
                    manual_pairings_next_round: swiss_data.manual_pairings_next_round.clone(),
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

        // Publish the result and the standings it produced. A bye is a scoring
        // adjustment with no board to report, so it never enters the log.
        if let Some(hub) = &self.braid_hub {
            if let Some(result_json) = match result {
                MatchResult::WhiteWin | MatchResult::ForfeitWhiteWin => {
                    Some(serde_json::json!({ "Win": { "winner": "white" } }))
                }
                MatchResult::BlackWin | MatchResult::ForfeitBlackWin => {
                    Some(serde_json::json!({ "Win": { "winner": "black" } }))
                }
                MatchResult::Draw => Some(serde_json::json!("Draw")),
                MatchResult::Bye => None,
            } {
                bridge::push_result(hub, tournament_id, round, board, result_json);
            }

            let standings_json = serde_json::to_value(&standings).unwrap_or_default();
            bridge::push_standings(hub, tournament_id, standings_json);
        }

        Ok(standings)
    }
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
                        bye_rounds: Vec::new(),
                        float_history: Vec::new(),
                        absent: swiss_data.absent_players.contains(id),
                        withdrawn: swiss_data.withdrawn_players.contains(id),
                        forfeit_round: None,
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

        // Handle byes — record which round each bye occurred in
        for round in &swiss_data.rounds {
            for bye_player_id in &round.byes {
                if let Some(player) = players.get_mut(bye_player_id) {
                    player.score += 1.0; // Bye = full point
                    player.bye_rounds.push(round.round);
                }
            }
        }

        // Rebuild float_history per round so consecutive float prevention works
        for round in &swiss_data.rounds {
            for (id, player) in players.iter_mut() {
                let float = if round.float_downs.contains(id) {
                    swiss_pairing::FloatStatus::Down
                } else if round.float_ups.contains(id) {
                    swiss_pairing::FloatStatus::Up
                } else {
                    swiss_pairing::FloatStatus::None
                };
                player.float_history.push(float);
            }
        }

        Ok(players.into_values().collect())
    }

    // ── Gap 1: Absent flag + forfeit ──────────────────────────────────────────

    pub async fn mark_absent(
        &self,
        tournament_id: u64,
        player_id: &str,
        round: u8,
    ) -> Result<(), SwissServiceError> {
        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let swiss_data = tournament
            .swiss_data
            .clone()
            .ok_or(SwissServiceError::NotSwissFormat)?;

        // Find any existing pairing for this player in the given round
        let forfeit_result: Option<(u16, MatchResult)> = swiss_data
            .rounds
            .iter()
            .find(|r| r.round == round)
            .and_then(|r| {
                r.pairings
                    .iter()
                    .find(|p| p.white == player_id || p.black == player_id)
            })
            .map(|p| {
                let result = if p.white == player_id {
                    MatchResult::ForfeitBlackWin // absent player was white → black wins
                } else {
                    MatchResult::ForfeitWhiteWin // absent player was black → white wins
                };
                (p.board, result)
            });

        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    if !sd.absent_players.contains(&player_id.to_string()) {
                        sd.absent_players.push(player_id.to_string());
                    }
                    if let Some((board, result)) = forfeit_result {
                        sd.results.push((round, board, result));
                    }
                }
            })
            .await;

        Ok(())
    }

    // ── Gap 2: Withdrawal timing distinction ──────────────────────────────────

    pub async fn withdraw_player(
        &self,
        tournament_id: u64,
        player_id: &str,
    ) -> Result<(), SwissServiceError> {
        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let swiss_data = tournament
            .swiss_data
            .clone()
            .ok_or(SwissServiceError::NotSwissFormat)?;

        let current_round = swiss_data.current_round;

        // Check if player is already paired in the current round
        let is_paired_this_round = swiss_data
            .rounds
            .iter()
            .find(|r| r.round == current_round)
            .map(|r| {
                r.pairings
                    .iter()
                    .any(|p| p.white == player_id || p.black == player_id)
            })
            .unwrap_or(false);

        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    if is_paired_this_round {
                        // Mid-round: mark absent (gap 1 handles the forfeit separately)
                        if !sd.absent_players.contains(&player_id.to_string()) {
                            sd.absent_players.push(player_id.to_string());
                        }
                    }
                    // Always mark withdrawn so they're excluded from future rounds
                    if !sd.withdrawn_players.contains(&player_id.to_string()) {
                        sd.withdrawn_players.push(player_id.to_string());
                    }
                }
            })
            .await;

        Ok(())
    }

    // ── Gap 3: Late rejoin ────────────────────────────────────────────────────

    pub async fn rejoin_player(
        &self,
        tournament_id: u64,
        player_id: &str,
    ) -> Result<(), SwissServiceError> {
        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let swiss_data = tournament
            .swiss_data
            .clone()
            .ok_or(SwissServiceError::NotSwissFormat)?;

        if swiss_data
            .withdrawn_players
            .contains(&player_id.to_string())
        {
            return Err(SwissServiceError::PlayerWithdrawn);
        }
        if !tournament.players.contains(&player_id.to_string()) {
            return Err(SwissServiceError::PlayerNotFound(player_id.to_string()));
        }

        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    sd.absent_players.retain(|id| id != player_id);
                }
            })
            .await;

        Ok(())
    }

    // ── Gap 6: Forbidden pairings ─────────────────────────────────────────────

    pub async fn add_forbidden_pair(
        &self,
        tournament_id: u64,
        player_a: &str,
        player_b: &str,
    ) -> Result<(), SwissServiceError> {
        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    let pair = (player_a.to_string(), player_b.to_string());
                    if !sd.forbidden_pairs.contains(&pair) {
                        sd.forbidden_pairs.push(pair);
                    }
                }
            })
            .await;
        Ok(())
    }

    pub async fn remove_forbidden_pair(
        &self,
        tournament_id: u64,
        player_a: &str,
        player_b: &str,
    ) -> Result<(), SwissServiceError> {
        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    sd.forbidden_pairs.retain(|(a, b)| {
                        !((a == player_a && b == player_b) || (a == player_b && b == player_a))
                    });
                }
            })
            .await;
        Ok(())
    }

    pub async fn add_manual_pairing(
        &self,
        tournament_id: u64,
        white: &str,
        black: &str,
    ) -> Result<(), SwissServiceError> {
        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    sd.manual_pairings_next_round.push(ManualPairing {
                        white: white.to_string(),
                        black: black.to_string(),
                    });
                }
            })
            .await;
        Ok(())
    }

    pub async fn remove_manual_pairing(
        &self,
        tournament_id: u64,
        white: &str,
        black: &str,
    ) -> Result<(), SwissServiceError> {
        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    sd.manual_pairings_next_round
                        .retain(|mp| !(mp.white == white && mp.black == black));
                }
            })
            .await;
        Ok(())
    }

    // ── Gap 7: Manual result override ─────────────────────────────────────────

    pub async fn override_result(
        &self,
        tournament_id: u64,
        round: u8,
        board: u16,
        new_result: MatchResult,
    ) -> Result<Vec<StandingsEntry>, SwissServiceError> {
        let tournament = self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?;

        let mut swiss_data = tournament
            .swiss_data
            .clone()
            .ok_or(SwissServiceError::NotSwissFormat)?;

        // Replace the existing result or push if not present
        let existing = swiss_data
            .results
            .iter_mut()
            .find(|(r, b, _)| *r == round && *b == board);

        if let Some(entry) = existing {
            entry.2 = new_result;
        } else {
            swiss_data.results.push((round, board, new_result));
        }

        // Recompute standings from scratch
        let players = self
            .build_swiss_players_with_results(&tournament, &swiss_data)
            .await?;
        let standings = calculate_standings(&players, &swiss_data.rounds, &swiss_data.results);
        swiss_data.standings = standings.clone();

        self.store
            .update(tournament_id, |t| {
                if let Some(ref mut sd) = t.swiss_data {
                    sd.results = swiss_data.results.clone();
                    sd.standings = standings.clone();
                }
            })
            .await;

        // Publish updated standings
        if let Some(hub) = &self.braid_hub {
            let standings_json = serde_json::to_value(&standings).unwrap_or_default();
            bridge::push_standings(hub, tournament_id, standings_json);
        }

        Ok(standings)
    }
}

// ── Scoring conversion (contract ↔ backend) ────────────────────────────────
//
// On-chain uses integer points (2/1/0), pairing engine uses FIDE float (1.0/0.5/0.0).
// See `SCORING.md` for the full mapping.

pub fn to_contract_points(score: f64) -> u8 {
    (score * 2.0).round() as u8
}

pub fn from_contract_points(points: u8) -> f64 {
    points as f64 / 2.0
}

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

    #[error("Player has permanently withdrawn and cannot rejoin")]
    PlayerWithdrawn,
}
