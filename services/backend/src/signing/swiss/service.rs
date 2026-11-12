use crate::signing::storage::tournament::{
    FormatState, GameOutcome, SwissStorageData, TournamentRecord, TournamentStatus, TournamentStore,
};
use std::collections::HashMap;
use std::sync::Arc;
use swiss_pairing::{
    calculate_standings, generate_pairings, Color, ManualPairing, MatchResult, PairingConfig,
    StandingsEntry, SwissPlayer, SwissRound,
};
use tracing::{info, warn};
use xfchess_braid_server::{bridge, ResourceHub};

/// Owns every Swiss state transition. Results come in through [`Self::record_result`]
/// (settled games, admin entry, forfeits); when a round's last board lands it pairs
/// the next round itself, and the final round's last board completes the event.
/// On-chain mirroring is the orchestrator's job and reads only what this persists.
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
                t.state = FormatState::Swiss(SwissStorageData::new(rounds));
                t.status = TournamentStatus::Active;
                t.started_at = Some(chrono::Utc::now().timestamp());
            })
            .await;

        Ok(())
    }

    /// Pairs the next round. Refuses while the current round still has open boards,
    /// so neither an admin nor a racing caller can skip an unplayed round.
    pub async fn start_round(&self, tournament_id: u64) -> Result<SwissRound, SwissServiceError> {
        let mut outcome: Result<SwissRound, SwissServiceError> =
            Err(SwissServiceError::TournamentNotFound);
        self.store
            .update(tournament_id, |t| {
                outcome = pair_next_round(t);
            })
            .await;
        let round = outcome?;

        info!(
            "Round {} started for tournament {}: {} pairings, {} byes",
            round.round,
            tournament_id,
            round.pairings.len(),
            round.byes.len()
        );

        if let Some(hub) = &self.braid_hub {
            let pairings: Vec<serde_json::Value> = round
                .pairings
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "board": p.board,
                        "white": p.white,
                        "black": p.black,
                        "game_id": TournamentStore::swiss_game_id(tournament_id, round.round, p.board),
                    })
                })
                .collect();
            bridge::push_pairings(
                hub,
                tournament_id,
                round.round,
                serde_json::Value::Array(pairings),
            );
        }

        Ok(round)
    }

    /// Records one board's result. Each board takes exactly one result here
    /// ([`Self::override_result`] corrects one); a duplicate — e.g. a settlement retry
    /// — is rejected with `ResultAlreadyRecorded`, which callers treat as done.
    pub async fn record_result(
        &self,
        tournament_id: u64,
        round: u8,
        board: u16,
        result: MatchResult,
    ) -> Result<Vec<StandingsEntry>, SwissServiceError> {
        info!(
            "Recording result for tournament {} round {} board {}: {:?}",
            tournament_id, round, board, result
        );

        let mut outcome: Result<(Vec<StandingsEntry>, bool, bool), SwissServiceError> =
            Err(SwissServiceError::TournamentNotFound);
        self.store
            .update(tournament_id, |t| {
                outcome = apply_result(t, round, board, result);
            })
            .await;
        let (standings, round_complete, completed) = outcome?;

        if completed {
            info!("Swiss tournament {} completed", tournament_id);
        } else if round_complete {
            match self.start_round(tournament_id).await {
                Ok(_) | Err(SwissServiceError::TournamentComplete) => {}
                Err(e) => warn!(
                    "Swiss tournament {}: pairing round {} failed: {}",
                    tournament_id,
                    round + 1,
                    e
                ),
            }
        }

        // A bye is a scoring adjustment with no board to report, so it never enters the log.
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

    /// Records a settled game: its id names the board, and its players must be the
    /// board's pairing.
    pub async fn record_game(
        &self,
        tournament_id: u64,
        game_id: u64,
        white: &str,
        black: &str,
        outcome: GameOutcome,
    ) -> Result<(u8, u16), SwissServiceError> {
        let (round, board) = TournamentStore::swiss_board_for_game_id(tournament_id, game_id)
            .ok_or(SwissServiceError::UnknownGame(game_id))?;
        let sd = self.swiss_data(tournament_id).await?;
        let pairing = sd
            .round(round)
            .and_then(|r| r.pairings.iter().find(|p| p.board == board))
            .ok_or(SwissServiceError::UnknownGame(game_id))?;
        if pairing.white != white || pairing.black != black {
            return Err(SwissServiceError::PlayersMismatch {
                game_id,
                white: white.to_string(),
                black: black.to_string(),
            });
        }
        self.record_result(tournament_id, round, board, outcome.into())
            .await?;
        Ok((round, board))
    }

    pub async fn get_pairings(
        &self,
        tournament_id: u64,
        round: u8,
    ) -> Result<Option<SwissRound>, SwissServiceError> {
        let swiss_data = self.swiss_data(tournament_id).await?;
        Ok(swiss_data.round(round).cloned())
    }

    pub async fn get_standings(
        &self,
        tournament_id: u64,
    ) -> Result<Vec<StandingsEntry>, SwissServiceError> {
        Ok(self.swiss_data(tournament_id).await?.standings)
    }

    pub async fn get_current_round(&self, tournament_id: u64) -> Result<u8, SwissServiceError> {
        Ok(self.swiss_data(tournament_id).await?.current_round)
    }

    pub async fn get_total_rounds(&self, tournament_id: u64) -> Result<u8, SwissServiceError> {
        Ok(self.swiss_data(tournament_id).await?.total_rounds)
    }

    async fn swiss_data(&self, tournament_id: u64) -> Result<SwissStorageData, SwissServiceError> {
        match self
            .store
            .get(tournament_id)
            .await
            .ok_or(SwissServiceError::TournamentNotFound)?
            .state
        {
            FormatState::Swiss(sd) => Ok(sd),
            FormatState::Elimination(_) => Err(SwissServiceError::NotSwissFormat),
        }
    }

    /// Marks a player absent and, if their board in `round` is still open, scores it
    /// as a forfeit through the normal result path (standings, round completion).
    pub async fn mark_absent(
        &self,
        tournament_id: u64,
        player_id: &str,
        round: u8,
    ) -> Result<(), SwissServiceError> {
        let swiss_data = self.swiss_data(tournament_id).await?;

        let forfeit: Option<(u16, MatchResult)> = swiss_data
            .round(round)
            .and_then(|r| {
                r.pairings
                    .iter()
                    .find(|p| p.white == player_id || p.black == player_id)
            })
            .filter(|p| !swiss_data.has_result(round, p.board))
            .map(|p| {
                let result = if p.white == player_id {
                    MatchResult::ForfeitBlackWin
                } else {
                    MatchResult::ForfeitWhiteWin
                };
                (p.board, result)
            });

        self.store
            .update(tournament_id, |t| {
                if let Some(sd) = t.swiss_mut() {
                    if !sd.absent_players.iter().any(|p| p == player_id) {
                        sd.absent_players.push(player_id.to_string());
                    }
                }
            })
            .await;

        if let Some((board, result)) = forfeit {
            match self
                .record_result(tournament_id, round, board, result)
                .await
            {
                Ok(_) | Err(SwissServiceError::ResultAlreadyRecorded) => {}
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    pub async fn withdraw_player(
        &self,
        tournament_id: u64,
        player_id: &str,
    ) -> Result<(), SwissServiceError> {
        let swiss_data = self.swiss_data(tournament_id).await?;
        let current_round = swiss_data.current_round;

        // A player withdrawing mid-round forfeits their open board.
        let is_paired_this_round = swiss_data.round(current_round).is_some_and(|r| {
            r.pairings
                .iter()
                .any(|p| p.white == player_id || p.black == player_id)
        });

        self.store
            .update(tournament_id, |t| {
                if let Some(sd) = t.swiss_mut() {
                    if !sd.withdrawn_players.iter().any(|p| p == player_id) {
                        sd.withdrawn_players.push(player_id.to_string());
                    }
                }
            })
            .await;

        if is_paired_this_round {
            self.mark_absent(tournament_id, player_id, current_round)
                .await?;
        }

        Ok(())
    }

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
            .swiss()
            .ok_or(SwissServiceError::NotSwissFormat)?;

        if swiss_data.withdrawn_players.iter().any(|p| p == player_id) {
            return Err(SwissServiceError::PlayerWithdrawn);
        }
        if !tournament.players.iter().any(|p| p == player_id) {
            return Err(SwissServiceError::PlayerNotFound(player_id.to_string()));
        }

        self.store
            .update(tournament_id, |t| {
                if let Some(sd) = t.swiss_mut() {
                    sd.absent_players.retain(|id| id != player_id);
                }
            })
            .await;

        Ok(())
    }

    pub async fn add_forbidden_pair(
        &self,
        tournament_id: u64,
        player_a: &str,
        player_b: &str,
    ) -> Result<(), SwissServiceError> {
        self.store
            .update(tournament_id, |t| {
                if let Some(sd) = t.swiss_mut() {
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
                if let Some(sd) = t.swiss_mut() {
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
                if let Some(sd) = t.swiss_mut() {
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
                if let Some(sd) = t.swiss_mut() {
                    sd.manual_pairings_next_round
                        .retain(|mp| !(mp.white == white && mp.black == black));
                }
            })
            .await;
        Ok(())
    }

    /// Replaces (or inserts) a board's result and recomputes standings. Only the
    /// off-chain record changes: a board already mirrored on-chain keeps its on-chain
    /// score, so overrides belong before the round closes.
    pub async fn override_result(
        &self,
        tournament_id: u64,
        round: u8,
        board: u16,
        new_result: MatchResult,
    ) -> Result<Vec<StandingsEntry>, SwissServiceError> {
        let mut outcome: Result<Vec<StandingsEntry>, SwissServiceError> =
            Err(SwissServiceError::TournamentNotFound);
        self.store
            .update(tournament_id, |t| {
                outcome = (|| {
                    let mut sd = t
                        .swiss()
                        .cloned()
                        .ok_or(SwissServiceError::NotSwissFormat)?;
                    match sd
                        .results
                        .iter_mut()
                        .find(|(r, b, _)| *r == round && *b == board)
                    {
                        Some(entry) => entry.2 = new_result,
                        None => sd.results.push((round, board, new_result)),
                    }
                    let players = build_swiss_players(t, &sd)?;
                    sd.standings = calculate_standings(&players, &sd.rounds, &sd.results);
                    let standings = sd.standings.clone();
                    if t.status == TournamentStatus::Completed {
                        apply_places(t, &standings);
                    }
                    t.state = FormatState::Swiss(sd);
                    Ok(standings)
                })();
            })
            .await;
        let standings = outcome?;

        if let Some(hub) = &self.braid_hub {
            let standings_json = serde_json::to_value(&standings).unwrap_or_default();
            bridge::push_standings(hub, tournament_id, standings_json);
        }

        Ok(standings)
    }
}

fn pair_next_round(t: &mut TournamentRecord) -> Result<SwissRound, SwissServiceError> {
    let sd = t.swiss().ok_or(SwissServiceError::NotSwissFormat)?;
    if t.status == TournamentStatus::Completed {
        return Err(SwissServiceError::TournamentComplete);
    }
    let next_round = sd.current_round + 1;
    if next_round > sd.total_rounds {
        return Err(SwissServiceError::TournamentComplete);
    }
    if sd.current_round > 0 && !sd.round_complete(sd.current_round) {
        return Err(SwissServiceError::RoundIncomplete(sd.current_round));
    }

    let players = build_swiss_players(t, sd)?;
    let config = PairingConfig {
        forbidden: sd.forbidden_pairs.clone(),
        manual_overrides: sd.manual_pairings_next_round.clone(),
    };
    let round = generate_pairings(next_round, &players, sd.total_rounds, &config)
        .map_err(|e| SwissServiceError::PairingError(e.to_string()))?;
    if round.pairings.len() > 0xff {
        return Err(SwissServiceError::PairingError(format!(
            "{} boards exceeds the 255-board limit",
            round.pairings.len()
        )));
    }

    let Some(sd) = t.swiss_mut() else {
        return Err(SwissServiceError::NotSwissFormat);
    };
    sd.current_round = next_round;
    sd.rounds.push(round.clone());
    sd.manual_pairings_next_round.clear();
    Ok(round)
}

/// Returns `(standings, round_complete, tournament_completed)`.
fn apply_result(
    t: &mut TournamentRecord,
    round: u8,
    board: u16,
    result: MatchResult,
) -> Result<(Vec<StandingsEntry>, bool, bool), SwissServiceError> {
    let mut sd = t
        .swiss()
        .cloned()
        .ok_or(SwissServiceError::NotSwissFormat)?;
    if round != sd.current_round {
        return Err(SwissServiceError::InvalidRound(round));
    }
    let round_data = sd
        .round(round)
        .ok_or(SwissServiceError::InvalidRound(round))?;
    if !round_data.pairings.iter().any(|p| p.board == board) {
        return Err(SwissServiceError::InvalidBoard(board));
    }
    if sd.has_result(round, board) {
        return Err(SwissServiceError::ResultAlreadyRecorded);
    }

    sd.results.push((round, board, result));
    let players = build_swiss_players(t, &sd)?;
    sd.standings = calculate_standings(&players, &sd.rounds, &sd.results);
    let standings = sd.standings.clone();

    let round_complete = sd.round_complete(round);
    let completed = round_complete && round >= sd.total_rounds;
    t.state = FormatState::Swiss(sd);

    if completed {
        t.status = TournamentStatus::Completed;
        t.completed_at = Some(chrono::Utc::now().timestamp());
        apply_places(t, &standings);
    }
    Ok((standings, round_complete, completed))
}

fn apply_places(t: &mut TournamentRecord, standings: &[StandingsEntry]) {
    let slots = [
        &mut t.winner,
        &mut t.second_place,
        &mut t.third_place,
        &mut t.fourth_place,
        &mut t.fifth_place,
        &mut t.sixth_place,
        &mut t.seventh_place,
        &mut t.eighth_place,
        &mut t.ninth_place,
        &mut t.tenth_place,
    ];
    for (i, slot) in slots.into_iter().enumerate() {
        *slot = standings.get(i).map(|s| s.player_id.clone());
    }
}

/// Rebuilds every player's score, colour history, opponents, byes and floats from
/// the persisted rounds and results — the pairing engine itself is stateless.
fn build_swiss_players(
    tournament: &TournamentRecord,
    swiss_data: &SwissStorageData,
) -> Result<Vec<SwissPlayer>, SwissServiceError> {
    let mut players: HashMap<String, SwissPlayer> = tournament
        .players
        .iter()
        .zip(tournament.player_elos.iter())
        .map(|(id, elo)| {
            let mut player = SwissPlayer::new(id.clone(), *elo);
            player.absent = swiss_data.absent_players.contains(id);
            player.withdrawn = swiss_data.withdrawn_players.contains(id);
            (id.clone(), player)
        })
        .collect();

    for (round_num, board, result) in &swiss_data.results {
        let pairing = swiss_data
            .round(*round_num)
            .ok_or(SwissServiceError::InvalidRound(*round_num))?
            .pairings
            .iter()
            .find(|p| p.board == *board)
            .ok_or(SwissServiceError::InvalidBoard(*board))?;

        if let Some(white) = players.get_mut(&pairing.white) {
            white.score += result.white_score();
            white.opponents.push(pairing.black.clone());
            white.color_history.push(Color::White);
        }
        if let Some(black) = players.get_mut(&pairing.black) {
            black.score += result.black_score();
            black.opponents.push(pairing.white.clone());
            black.color_history.push(Color::Black);
        }
    }

    for round in &swiss_data.rounds {
        for bye_player_id in &round.byes {
            if let Some(player) = players.get_mut(bye_player_id) {
                player.score += 1.0; // Bye = full point
                player.bye_rounds.push(round.round);
            }
        }
    }

    // Rebuild float_history per round so consecutive float prevention works.
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

// On-chain points are 2/1/0; the pairing engine uses 1.0/0.5/0.0.

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

    #[error("Round {0} still has boards without a result")]
    RoundIncomplete(u8),

    #[error("This board already has a result")]
    ResultAlreadyRecorded,

    #[error("Pairing error: {0}")]
    PairingError(String),

    #[error("Player not found: {0}")]
    PlayerNotFound(String),

    #[error("Invalid result format")]
    InvalidResult,

    #[error("Player has permanently withdrawn and cannot rejoin")]
    PlayerWithdrawn,

    #[error("game {0} is not a board of this event")]
    UnknownGame(u64),

    #[error("game {game_id} was played by {white} / {black}, not by the board's pairing")]
    PlayersMismatch {
        game_id: u64,
        white: String,
        black: String,
    },
}
