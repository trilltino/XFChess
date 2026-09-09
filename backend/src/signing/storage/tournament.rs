use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentFormat {
    SingleElimination,
    Swiss { rounds: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwissStorageData {
    pub current_round: u8,
    pub total_rounds: u8,
    pub rounds: Vec<swiss_pairing::SwissRound>,
    pub results: Vec<(u8, u16, swiss_pairing::MatchResult)>,
    pub standings: Vec<swiss_pairing::StandingsEntry>,
    #[serde(default)]
    pub round_deadline_at: Option<i64>,
    #[serde(default)]
    pub absent_players: Vec<String>,
    #[serde(default)]
    pub withdrawn_players: Vec<String>,
    #[serde(default)]
    pub forbidden_pairs: Vec<(String, String)>,
    #[serde(default)]
    pub manual_pairings_next_round: Vec<swiss_pairing::ManualPairing>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentStatus {
    Registration,
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchStatus {
    Pending,
    Active,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResultSource {
    OnChain,
    Oracle,
    Forfeit,
    DrawAgreed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentMatch {
    pub match_index: u16,
    pub round: u8,
    pub player_white: Option<String>,
    pub player_black: Option<String>,
    pub winner: Option<String>,
    pub game_id: Option<u64>,
    pub status: MatchStatus,
    pub result_source: Option<ResultSource>,
    pub next_match_for_winner: Option<u16>,
    pub next_match_slot: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentRecord {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub platform_fee_lamports: u64,
    pub prize_pool: u64,
    pub max_players: u16,
    pub status: TournamentStatus,
    pub format: TournamentFormat,
    pub players: Vec<String>,
    pub player_elos: Vec<u32>,
    pub node_ids: HashMap<String, String>,
    pub matches: Vec<Option<TournamentMatch>>,
    pub winner: Option<String>,
    pub second_place: Option<String>,
    pub third_place: Option<String>,
    pub fourth_place: Option<String>,
    pub fifth_place: Option<String>,
    pub sixth_place: Option<String>,
    pub seventh_place: Option<String>,
    pub eighth_place: Option<String>,
    pub ninth_place: Option<String>,
    pub tenth_place: Option<String>,
    pub prize_shares: [u16; 10],
    pub swiss_data: Option<SwissStorageData>,
    pub elo_min: Option<u32>,
    pub elo_max: Option<u32>,
    pub min_players: Option<u16>,
    pub created_at: i64,
    pub scheduled_at: Option<i64>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    #[serde(default)]
    pub kyc_required: bool,
    pub password_hash: Option<String>,
    #[serde(default)]
    pub prizes_distributed: bool,
    #[serde(default)]
    pub prize_release_approved: bool,
    #[serde(default)]
    pub broadcast_delay_secs: u32,
}

impl TournamentRecord {
    pub fn new(tournament_id: u64, name: &str, entry_fee_lamports: u64) -> Self {
        Self {
            tournament_id,
            name: name.to_string(),
            entry_fee_lamports,
            platform_fee_lamports: 4_000_000, // Default to 50p
            prize_pool: 0,
            max_players: 8,
            status: TournamentStatus::Registration,
            format: TournamentFormat::SingleElimination,
            players: Vec::new(),
            player_elos: Vec::new(),
            node_ids: HashMap::new(),
            matches: vec![None; 7],
            winner: None,
            second_place: None,
            third_place: None,
            fourth_place: None,
            fifth_place: None,
            sixth_place: None,
            seventh_place: None,
            eighth_place: None,
            ninth_place: None,
            tenth_place: None,
            password_hash: None,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            min_players: None,
            swiss_data: None,
            prize_shares: [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            elo_min: None,
            elo_max: None,
            created_at: chrono::Utc::now().timestamp(),
            kyc_required: false,
            prizes_distributed: false,
            prize_release_approved: false,
            broadcast_delay_secs: 0,
        }
    }

    pub fn with_config(
        tournament_id: u64,
        name: String,
        entry_fee_lamports: u64,
        platform_fee_lamports: u64,
        max_players: u16,
        prize_shares: [u16; 10],
        format: TournamentFormat,
        elo_min: Option<u32>,
        elo_max: Option<u32>,
        min_players: Option<u16>,
        scheduled_at: Option<i64>,
        kyc_required: bool,
    ) -> Self {
        let total_matches = (max_players - 1) as usize;
        Self {
            tournament_id,
            name,
            entry_fee_lamports,
            platform_fee_lamports,
            prize_pool: 0,
            max_players,
            status: TournamentStatus::Registration,
            format,
            players: Vec::with_capacity(max_players as usize),
            player_elos: Vec::with_capacity(max_players as usize),
            node_ids: HashMap::new(),
            matches: vec![None; total_matches],
            winner: None,
            second_place: None,
            third_place: None,
            fourth_place: None,
            fifth_place: None,
            sixth_place: None,
            seventh_place: None,
            eighth_place: None,
            ninth_place: None,
            tenth_place: None,
            password_hash: None,
            scheduled_at,
            started_at: None,
            completed_at: None,
            min_players,
            swiss_data: None,
            prize_shares,
            elo_min,
            elo_max,
            created_at: chrono::Utc::now().timestamp(),
            kyc_required,
            prizes_distributed: false,
            prize_release_approved: false,
            broadcast_delay_secs: 0,
        }
    }

    pub fn is_full(&self) -> bool {
        self.players.len() >= self.max_players as usize
    }

    pub fn current_round(&self) -> Option<u8> {
        self.matches
            .iter()
            .flatten()
            .filter(|m| m.status != MatchStatus::Completed)
            .map(|m| m.round)
            .min()
    }

    pub fn matches_remaining_in_round(&self, round: u8) -> usize {
        self.matches
            .iter()
            .flatten()
            .filter(|m| m.round == round && m.status != MatchStatus::Completed)
            .count()
    }

    pub fn total_rounds(&self) -> u8 {
        match self.format {
            TournamentFormat::Swiss { rounds } => rounds,
            TournamentFormat::SingleElimination => self
                .matches
                .iter()
                .flatten()
                .map(|m| m.round)
                .max()
                .map(|r| r + 1)
                .unwrap_or(0),
        }
    }

    fn last_finished_match(&self, player: &str) -> Option<(u8, bool, String)> {
        self.matches
            .iter()
            .flatten()
            .filter(|m| m.status == MatchStatus::Completed)
            .filter(|m| {
                m.player_white.as_deref() == Some(player)
                    || m.player_black.as_deref() == Some(player)
            })
            .max_by_key(|m| m.round)
            .map(|m| {
                let won = m.winner.as_deref() == Some(player);
                let opponent = if m.player_white.as_deref() == Some(player) {
                    m.player_black.clone().unwrap_or_default()
                } else {
                    m.player_white.clone().unwrap_or_default()
                };
                (m.round, won, opponent)
            })
    }

    fn placing_for(&self, player: &str) -> Option<u8> {
        let places = [
            &self.winner,
            &self.second_place,
            &self.third_place,
            &self.fourth_place,
            &self.fifth_place,
            &self.sixth_place,
            &self.seventh_place,
            &self.eighth_place,
            &self.ninth_place,
            &self.tenth_place,
        ];
        places
            .iter()
            .position(|p| p.as_deref() == Some(player))
            .map(|i| (i + 1) as u8)
    }

    pub fn player_status(&self, player: &str) -> PlayerTournamentStatus {
        let registered = self.players.iter().any(|p| p == player);
        let current_round = self.current_round();
        let total_rounds = self.total_rounds();
        let last_result = self
            .last_finished_match(player)
            .map(|(round, won, opponent)| LastMatchResult {
                round,
                won,
                opponent,
            });

        let mut status = PlayerTournamentStatus {
            state: PlayerState::NotRegistered,
            registered,
            tournament_status: self.status.clone(),
            round: current_round,
            total_rounds,
            r#match: None,
            blocked_by: None,
            last_result,
            placing: None,
            prize_lamports: None,
        };

        if !registered {
            return status;
        }

        match self.status {
            TournamentStatus::Registration => {
                status.state = PlayerState::Registered;
                return status;
            }
            TournamentStatus::Cancelled => {
                status.state = PlayerState::Cancelled;
                return status;
            }
            TournamentStatus::Completed => {
                let placing = self.placing_for(player);
                status.placing = placing;
                status.prize_lamports = placing.map(|p| self.calculate_prize(p));
                status.state = if placing == Some(1) {
                    PlayerState::Champion
                } else {
                    PlayerState::Eliminated
                };
                return status;
            }
            TournamentStatus::Active => {}
        }

        // Active: is the player seated in an unfinished match?
        if let Some(assignment) = self.match_for_player(player) {
            status.state = if assignment.game_id.is_some() {
                PlayerState::MatchReady
            } else {
                // Both players known but the scheduler hasn't stamped a game
                // ID yet — a brief window, not an error.
                PlayerState::AwaitingGameId
            };
            status.r#match = Some(assignment);
            return status;
        }

        // No playable match. Either they were knocked out, or they advanced
        // and their next opponent is still being decided.
        let knocked_out = self.matches.iter().flatten().any(|m| {
            m.status == MatchStatus::Completed
                && (m.player_white.as_deref() == Some(player)
                    || m.player_black.as_deref() == Some(player))
                && m.winner.as_deref() != Some(player)
        });

        if knocked_out {
            status.state = PlayerState::Eliminated;
            status.placing = self.placing_for(player);
            status.prize_lamports = status.placing.map(|p| self.calculate_prize(p));
        } else {
            status.state = PlayerState::AwaitingOpponent;
            if let Some(round) = current_round {
                status.blocked_by = Some(BlockedBy {
                    round,
                    matches_remaining: self.matches_remaining_in_round(round),
                });
            }
        }
        status
    }

    pub fn final_match_index(&self) -> usize {
        self.matches.len() - 1
    }

    pub fn semifinal1_index(&self) -> usize {
        self.final_match_index().saturating_sub(2)
    }

    pub fn semifinal2_index(&self) -> usize {
        self.final_match_index().saturating_sub(1)
    }

    pub fn match_for_player(&self, player: &str) -> Option<MatchAssignment> {
        if matches!(self.format, TournamentFormat::Swiss { .. }) {
            return self.swiss_match_for_player(player);
        }

        for m in self.matches.iter().flatten() {
            if m.status == MatchStatus::Completed {
                continue;
            }
            let is_white = m.player_white.as_deref() == Some(player);
            let is_black = m.player_black.as_deref() == Some(player);
            if !is_white && !is_black {
                continue;
            }
            let opponent = if is_white {
                m.player_black.clone()?
            } else {
                m.player_white.clone()?
            };
            let opponent_node_id = self.node_ids.get(&opponent).cloned();
            return Some(MatchAssignment {
                match_index: m.match_index,
                round: Some(m.round),
                board: None,
                game_id: m.game_id,
                opponent_pubkey: opponent,
                opponent_node_id,
                your_color: if is_white { "white" } else { "black" }.to_string(),
                status: m.status.clone(),
                is_bye: false,
            });
        }
        None
    }

    fn swiss_match_for_player(&self, player: &str) -> Option<MatchAssignment> {
        let swiss = self.swiss_data.as_ref()?;
        let round = swiss.rounds.last()?;

        if round.byes.iter().any(|bye| bye == player) {
            return Some(MatchAssignment {
                match_index: 0,
                round: Some(round.round),
                board: None,
                game_id: None,
                opponent_pubkey: String::new(),
                opponent_node_id: None,
                your_color: "bye".to_string(),
                status: MatchStatus::Completed,
                is_bye: true,
            });
        }

        let pairing = round
            .pairings
            .iter()
            .enumerate()
            .find(|(_, p)| p.white == player || p.black == player)?;

        let (idx, p) = pairing;
        let is_white = p.white == player;
        let opponent = if is_white {
            p.black.clone()
        } else {
            p.white.clone()
        };

        Some(MatchAssignment {
            match_index: idx as u16,
            round: Some(round.round),
            board: Some(p.board),
            game_id: None,
            opponent_node_id: self.node_ids.get(&opponent).cloned(),
            opponent_pubkey: opponent,
            your_color: if is_white { "white" } else { "black" }.to_string(),
            status: MatchStatus::Active,
            is_bye: false,
        })
    }

    pub fn generate_bracket(&mut self) {
        let player_count = self.players.len();
        if player_count < 2 {
            self.matches.clear();
            return;
        }
        let max_players = player_count as u16;
        let total_matches = player_count - 1;
        let round1_matches = player_count / 2;

        self.matches = (0..total_matches)
            .map(|i| {
                let (round, next_match, next_slot) =
                    crate::signing::solana::bracket_position(max_players, i as u16);

                // Round-1 matches are seeded directly from the ELO-sorted
                // player list (highest vs lowest); every later round starts
                // empty and gets filled in as winners are recorded.
                let (player_white, player_black) = if i < round1_matches {
                    (
                        Some(self.players[i].clone()),
                        Some(self.players[player_count - 1 - i].clone()),
                    )
                } else {
                    (None, None)
                };

                Some(TournamentMatch {
                    match_index: i as u16,
                    round,
                    player_white,
                    player_black,
                    winner: None,
                    game_id: None,
                    status: MatchStatus::Pending,
                    result_source: None,
                    next_match_for_winner: next_match,
                    next_match_slot: next_slot,
                })
            })
            .collect();
    }

    pub fn calculate_prize(&self, place: u8) -> u64 {
        let share_bps = match place {
            1 => self.prize_shares[0],
            2 => self.prize_shares[1],
            3 => self.prize_shares[2],
            4 => self.prize_shares[3],
            _ => 0,
        };

        if share_bps == 0 {
            return 0;
        }

        (self.prize_pool as u128 * share_bps as u128 / 10000) as u64
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PlayerState {
    NotRegistered,
    Registered,
    MatchReady,
    AwaitingGameId,
    AwaitingOpponent,
    Eliminated,
    Champion,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedBy {
    pub round: u8,
    pub matches_remaining: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastMatchResult {
    pub round: u8,
    pub won: bool,
    pub opponent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerTournamentStatus {
    pub state: PlayerState,
    pub registered: bool,
    pub tournament_status: TournamentStatus,
    pub round: Option<u8>,
    pub total_rounds: u8,
    pub r#match: Option<MatchAssignment>,
    pub blocked_by: Option<BlockedBy>,
    pub last_result: Option<LastMatchResult>,
    pub placing: Option<u8>,
    pub prize_lamports: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchAssignment {
    pub match_index: u16,
    pub round: Option<u8>,
    pub board: Option<u16>,
    pub game_id: Option<u64>,
    pub opponent_pubkey: String,
    pub opponent_node_id: Option<String>,
    pub your_color: String,
    pub status: MatchStatus,
    pub is_bye: bool,
}

#[derive(Clone)]
pub struct TournamentStore {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationTransaction {
    pub tournament_id: u64,
    pub player: String,
    pub elo: u32,
    pub signature: String,
    pub confirmed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentTransaction {
    pub tournament_id: u64,
    pub signature: String,
    pub operation: String,
    pub status: String,
    pub retry_count: u32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<i64>,
    pub created_at: i64,
}

impl TournamentStore {
    pub async fn new(pool: SqlitePool) -> Self {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tournaments (
                id       INTEGER PRIMARY KEY,
                data     TEXT    NOT NULL,
                updated_at INTEGER NOT NULL DEFAULT 0
            );",
        )
        .execute(&pool)
        .await
        .ok();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tournament_registration_transactions (
                tournament_id INTEGER NOT NULL,
                player TEXT NOT NULL,
                elo INTEGER NOT NULL,
                signature TEXT NOT NULL,
                confirmed_at INTEGER NOT NULL,
                PRIMARY KEY (tournament_id, player)
            );",
        )
        .execute(&pool)
        .await
        .ok();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tournament_transactions (
                tournament_id INTEGER NOT NULL,
                signature TEXT PRIMARY KEY,
                operation TEXT NOT NULL,
                status TEXT NOT NULL,
                retry_count INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                next_retry_at INTEGER,
                created_at INTEGER NOT NULL
            );",
        )
        .execute(&pool)
        .await
        .ok();
        tracing::info!("[tournament-store] SQLite table ready");
        Self { pool }
    }

    pub async fn record_registration_transaction(&self, tx: RegistrationTransaction) -> bool {
        sqlx::query(
            "INSERT OR REPLACE INTO tournament_registration_transactions
             (tournament_id, player, elo, signature, confirmed_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(tx.tournament_id as i64)
        .bind(tx.player)
        .bind(tx.elo as i64)
        .bind(tx.signature)
        .bind(tx.confirmed_at)
        .execute(&self.pool)
        .await
        .is_ok()
    }

    pub async fn record_transaction(&self, tx: TournamentTransaction) -> bool {
        sqlx::query(
            "INSERT OR REPLACE INTO tournament_transactions
             (tournament_id, signature, operation, status, retry_count, last_error, next_retry_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(tx.tournament_id as i64)
        .bind(tx.signature)
        .bind(tx.operation)
        .bind(tx.status)
        .bind(tx.retry_count as i64)
        .bind(tx.last_error)
        .bind(tx.next_retry_at)
        .bind(tx.created_at)
        .execute(&self.pool)
        .await
        .is_ok()
    }

    pub async fn transactions(&self, id: u64) -> Vec<TournamentTransaction> {
        sqlx::query_as::<_, (i64, String, String, String, i64, Option<String>, Option<i64>, i64)>(
            "SELECT tournament_id, signature, operation, status, retry_count, last_error, next_retry_at, created_at
             FROM tournament_transactions WHERE tournament_id = ? ORDER BY created_at DESC",
        )
        .bind(id as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(tournament_id, signature, operation, status, retry_count, last_error, next_retry_at, created_at)| TournamentTransaction {
            tournament_id: tournament_id as u64,
            signature,
            operation,
            status,
            retry_count: retry_count as u32,
            last_error,
            next_retry_at,
            created_at,
        })
        .collect()
    }

    pub async fn registration_transactions(&self, id: u64) -> Vec<RegistrationTransaction> {
        sqlx::query_as::<_, (i64, String, i64, String, i64)>(
            "SELECT tournament_id, player, elo, signature, confirmed_at
             FROM tournament_registration_transactions WHERE tournament_id = ?
             ORDER BY confirmed_at ASC",
        )
        .bind(id as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(
            |(tournament_id, player, elo, signature, confirmed_at)| RegistrationTransaction {
                tournament_id: tournament_id as u64,
                player,
                elo: elo as u32,
                signature,
                confirmed_at,
            },
        )
        .collect()
    }

    pub async fn create(&self, record: TournamentRecord) {
        let data = serde_json::to_string(&record).unwrap_or_default();
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT OR REPLACE INTO tournaments (id, data, updated_at) VALUES (?, ?, ?)")
            .bind(record.tournament_id as i64)
            .bind(&data)
            .bind(now)
            .execute(&self.pool)
            .await
            .ok();
    }

    pub async fn get(&self, id: u64) -> Option<TournamentRecord> {
        let row = sqlx::query("SELECT data FROM tournaments WHERE id = ?")
            .bind(id as i64)
            .fetch_optional(&self.pool)
            .await
            .ok()??;
        serde_json::from_str(&row.get::<String, _>(0)).ok()
    }

    pub async fn list(&self) -> Vec<TournamentRecord> {
        let rows = sqlx::query("SELECT data FROM tournaments")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_else(|e| {
                tracing::error!("Failed to fetch tournaments from database: {}", e);
                Vec::new()
            });
        rows.into_iter()
            .filter_map(|r| serde_json::from_str::<TournamentRecord>(&r.get::<String, _>(0)).ok())
            .collect()
    }

    pub async fn delete(&self, id: u64) -> bool {
        sqlx::query("DELETE FROM tournaments WHERE id = ?")
            .bind(id as i64)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
            .unwrap_or(false)
    }

    pub async fn update<F: FnOnce(&mut TournamentRecord)>(&self, id: u64, f: F) -> bool {
        if let Some(mut record) = self.get(id).await {
            f(&mut record);
            let data = serde_json::to_string(&record).unwrap_or_default();
            let now = chrono::Utc::now().timestamp();
            sqlx::query("UPDATE tournaments SET data = ?, updated_at = ? WHERE id = ?")
                .bind(data)
                .bind(now)
                .bind(id as i64)
                .execute(&self.pool)
                .await
                .is_ok()
        } else {
            false
        }
    }

    pub async fn register_node_id(&self, id: u64, player: String, node_id: String) -> bool {
        self.update(id, |t| {
            t.node_ids.insert(player, node_id);
        })
        .await
    }

    pub async fn leave_tournament(&self, id: u64, player: &str) -> bool {
        self.update(id, |t| {
            if let Some(pos) = t.players.iter().position(|p| p == player) {
                t.players.remove(pos);
                t.player_elos.remove(pos);
                if t.prize_pool >= t.entry_fee_lamports {
                    t.prize_pool -= t.entry_fee_lamports;
                }
            }
        })
        .await
    }

    pub async fn set_match_game_id(&self, id: u64, match_index: usize, game_id: u64) -> bool {
        self.update(id, |t| {
            if let Some(m) = t.matches[match_index].as_mut() {
                m.game_id = Some(game_id);
                m.status = MatchStatus::Active;
            }
        })
        .await
    }

    pub fn deterministic_game_id(tournament_id: u64, match_index: u16) -> Option<u64> {
        (tournament_id < (1u64 << 48)).then_some((tournament_id << 16) | u64::from(match_index))
    }

    pub async fn assign_ready_game_ids(&self, id: u64) -> Vec<(u16, u64)> {
        let mut assigned = Vec::new();
        self.update(id, |t| {
            for (index, maybe_match) in t.matches.iter_mut().enumerate() {
                let Some(m) = maybe_match else { continue };
                if m.game_id.is_some() || m.player_white.is_none() || m.player_black.is_none() {
                    continue;
                }
                let Some(game_id) = Self::deterministic_game_id(id, m.match_index) else {
                    continue;
                };
                m.game_id = Some(game_id);
                m.status = MatchStatus::Active;
                assigned.push((index as u16, game_id));
            }
        })
        .await;
        assigned
    }

    pub async fn record_result(
        &self,
        id: u64,
        match_index: usize,
        winner: String,
        loser: String,
    ) -> bool {
        let updated = self
            .update(id, |t| {
                if let Some(m) = t.matches[match_index].as_mut() {
                    m.winner = Some(winner.clone());
                    m.status = MatchStatus::Completed;
                }

                // Advance the winner into their next-round match slot (if any).
                let next = t.matches[match_index].as_ref().and_then(|m| {
                    m.next_match_for_winner
                        .map(|n| (n as usize, m.next_match_slot))
                });
                if let Some((next_idx, slot)) = next {
                    if next_idx < t.matches.len() {
                        if let Some(nm) = t.matches[next_idx].as_mut() {
                            if slot == 0 {
                                nm.player_white = Some(winner.clone());
                            } else {
                                nm.player_black = Some(winner.clone());
                            }
                        }
                    }
                }

                let final_idx = t.final_match_index();

                // The final must be checked before the semifinals: a 2-player
                // bracket has a single match, so the saturating semifinal indices
                // would otherwise swallow the final and never complete the
                // tournament. Semifinals only exist in brackets of 4+ players.
                if match_index == final_idx {
                    // Final complete - tournament done
                    t.winner = Some(winner);
                    t.second_place = Some(loser);
                    t.status = TournamentStatus::Completed;
                    t.completed_at = Some(chrono::Utc::now().timestamp());
                } else if t.matches.len() >= 3 && match_index == t.semifinal1_index() {
                    // First semifinal - loser is 4th place
                    t.fourth_place = Some(loser);
                } else if t.matches.len() >= 3 && match_index == t.semifinal2_index() {
                    // Second semifinal - loser is 3rd place
                    t.third_place = Some(loser);
                }
            })
            .await;
        if updated {
            self.assign_ready_game_ids(id).await;
        }
        updated
    }

    pub async fn update_status(&self, id: u64, status: TournamentStatus) -> bool {
        self.update(id, |t| {
            t.status = status;
        })
        .await
    }

    pub async fn seed_players_by_elo(&self, id: u64) -> bool {
        self.update(id, |t| {
            let mut indexed: Vec<(usize, u32)> =
                t.player_elos.iter().copied().enumerate().collect();
            indexed.sort_by(|a, b| b.1.cmp(&a.1)); // descending ELO

            // Reorder players and elos by sorted index
            let mut sorted_players = Vec::new();
            let mut sorted_elos = Vec::new();
            for (idx, _) in indexed {
                sorted_players.push(t.players[idx].clone());
                sorted_elos.push(t.player_elos[idx]);
            }
            t.players = sorted_players;
            t.player_elos = sorted_elos;
        })
        .await
    }

    pub async fn generate_bracket(&self, id: u64) -> bool {
        self.update(id, |t| {
            if t.format != TournamentFormat::SingleElimination {
                return;
            }
            // Builds every round up front (later-round matches start with empty
            // player slots); record_result advances winners into them.
            t.generate_bracket();
        })
        .await
    }

    pub async fn start_tournament(&self, id: u64) -> Result<(), String> {
        let tournament = self.get(id).await.ok_or("Tournament not found")?;

        // Seed players first
        if !self.seed_players_by_elo(id).await {
            return Err("Failed to seed players".to_string());
        }

        match tournament.format {
            TournamentFormat::SingleElimination => {
                if !self.generate_bracket(id).await {
                    return Err("Failed to generate bracket".to_string());
                }
            }
            TournamentFormat::Swiss { .. } => {
                // Swiss bracket generation handled by swiss service
                // Just verify we have enough players for at least one pairing
                if tournament.players.len() < tournament.min_players.unwrap_or(2) as usize {
                    return Err("Not enough players for Swiss tournament".to_string());
                }
            }
        }

        // Set status to Active
        if !self.update_status(id, TournamentStatus::Active).await {
            return Err("Failed to update tournament status".to_string());
        }

        // Set start time
        self.update(id, |t| {
            t.started_at = Some(chrono::Utc::now().timestamp());
        })
        .await;

        Ok(())
    }
}

impl Default for TournamentRecord {
    fn default() -> Self {
        Self {
            tournament_id: 0,
            name: String::new(),
            entry_fee_lamports: 0,
            platform_fee_lamports: 4_000_000,
            prize_pool: 0,
            max_players: 8,
            status: TournamentStatus::Registration,
            format: TournamentFormat::SingleElimination,
            players: Vec::new(),
            player_elos: Vec::new(),
            node_ids: HashMap::new(),
            matches: vec![None; 7],
            winner: None,
            second_place: None,
            third_place: None,
            fourth_place: None,
            fifth_place: None,
            sixth_place: None,
            seventh_place: None,
            eighth_place: None,
            ninth_place: None,
            tenth_place: None,
            password_hash: None,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            min_players: None,
            swiss_data: None,
            prize_shares: [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            elo_min: None,
            elo_max: None,
            created_at: chrono::Utc::now().timestamp(),
            kyc_required: false,
            prizes_distributed: false,
            prize_release_approved: false,
            broadcast_delay_secs: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with_players(tournament_id: u64, n: usize) -> TournamentRecord {
        let mut t = TournamentRecord::new(tournament_id, "test", 0);
        t.max_players = n as u16;
        for i in 0..n {
            t.players.push(format!("P{i}"));
            t.player_elos.push(2000 - i as u32); // P0 highest seed
        }
        t
    }

    async fn mem_store() -> TournamentStore {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        TournamentStore::new(pool).await
    }

    #[test]
    fn bracket_two_players_is_a_single_final() {
        let mut t = record_with_players(1, 2);
        t.generate_bracket();

        assert_eq!(t.matches.len(), 1);
        let m = t.matches[0].as_ref().unwrap();
        assert_eq!(m.round, 0);
        assert_eq!(m.player_white.as_deref(), Some("P0"));
        assert_eq!(m.player_black.as_deref(), Some("P1"));
        // The only match is the final — it must not point past the bracket.
        assert_eq!(m.next_match_for_winner, None);
    }

    #[test]
    fn bracket_four_players_semis_feed_the_final() {
        let mut t = record_with_players(1, 4);
        t.generate_bracket();

        assert_eq!(t.matches.len(), 3);
        // Semifinal 0: seed 1 vs seed 4, winner goes to final slot 0
        let m0 = t.matches[0].as_ref().unwrap();
        assert_eq!(m0.player_white.as_deref(), Some("P0"));
        assert_eq!(m0.player_black.as_deref(), Some("P3"));
        assert_eq!(m0.next_match_for_winner, Some(2));
        assert_eq!(m0.next_match_slot, 0);
        // Semifinal 1: seed 2 vs seed 3, winner goes to final slot 1
        let m1 = t.matches[1].as_ref().unwrap();
        assert_eq!(m1.player_white.as_deref(), Some("P1"));
        assert_eq!(m1.player_black.as_deref(), Some("P2"));
        assert_eq!(m1.next_match_for_winner, Some(2));
        assert_eq!(m1.next_match_slot, 1);
        // Final exists as an empty shell awaiting winners
        let m2 = t.matches[2].as_ref().unwrap();
        assert_eq!(m2.round, 1);
        assert!(m2.player_white.is_none() && m2.player_black.is_none());
        assert_eq!(m2.next_match_for_winner, None);
    }

    #[test]
    fn bracket_eight_players_next_pointers_stay_in_bounds() {
        let mut t = record_with_players(1, 8);
        t.generate_bracket();

        assert_eq!(t.matches.len(), 7);
        for m in t.matches.iter().flatten() {
            if let Some(next) = m.next_match_for_winner {
                assert!((next as usize) < t.matches.len());
                assert!(next > m.match_index);
            } else {
                assert_eq!(m.match_index, 6); // only the final has no successor
            }
        }
        // Semifinal winners meet in the final
        assert_eq!(
            t.matches[4].as_ref().unwrap().next_match_for_winner,
            Some(6)
        );
        assert_eq!(
            t.matches[5].as_ref().unwrap().next_match_for_winner,
            Some(6)
        );
    }

    #[test]
    fn deterministic_game_ids_are_stable_and_bounded() {
        assert_eq!(
            TournamentStore::deterministic_game_id(42, 7),
            Some((42 << 16) | 7)
        );
        assert_eq!(TournamentStore::deterministic_game_id(1 << 48, 0), None);
    }

    #[tokio::test]
    async fn two_player_tournament_completes_on_its_only_match() {
        let store = mem_store().await;
        let mut t = record_with_players(11, 2);
        t.generate_bracket();
        t.status = TournamentStatus::Active;
        store.create(t).await;

        assert!(store.record_result(11, 0, "P1".into(), "P0".into()).await);

        let t = store.get(11).await.unwrap();
        assert_eq!(t.status, TournamentStatus::Completed);
        assert_eq!(t.winner.as_deref(), Some("P1"));
        assert_eq!(t.second_place.as_deref(), Some("P0"));
        // A head-to-head has no semifinals — no phantom 3rd/4th placements.
        assert!(t.third_place.is_none());
        assert!(t.fourth_place.is_none());
    }

    #[tokio::test]
    async fn four_player_tournament_advances_winners_and_completes() {
        let store = mem_store().await;
        let mut t = record_with_players(12, 4);
        t.generate_bracket();
        t.status = TournamentStatus::Active;
        store.create(t).await;

        // Semifinal 0: P0 beats P3 (P3 -> 4th)
        assert!(store.record_result(12, 0, "P0".into(), "P3".into()).await);
        // Semifinal 1: P1 beats P2 (P2 -> 3rd)
        assert!(store.record_result(12, 1, "P1".into(), "P2".into()).await);

        // Both winners must have been advanced into the final.
        let t = store.get(12).await.unwrap();
        let final_match = t.matches[2].as_ref().unwrap();
        assert_eq!(final_match.player_white.as_deref(), Some("P0"));
        assert_eq!(final_match.player_black.as_deref(), Some("P1"));
        assert_eq!(t.status, TournamentStatus::Active);
        assert_eq!(t.fourth_place.as_deref(), Some("P3"));
        assert_eq!(t.third_place.as_deref(), Some("P2"));

        // Final: P1 beats P0
        assert!(store.record_result(12, 2, "P1".into(), "P0".into()).await);
        let t = store.get(12).await.unwrap();
        assert_eq!(t.status, TournamentStatus::Completed);
        assert_eq!(t.winner.as_deref(), Some("P1"));
        assert_eq!(t.second_place.as_deref(), Some("P0"));
    }
}

#[cfg(test)]
mod player_status_tests {
    use super::*;

    fn record_with_players(tournament_id: u64, n: usize) -> TournamentRecord {
        let mut t = TournamentRecord::new(tournament_id, "test", 0);
        t.max_players = n as u16;
        for i in 0..n {
            t.players.push(format!("P{i}"));
            t.player_elos.push(2000 - i as u32);
        }
        t
    }

    #[test]
    fn winner_awaiting_opponent_is_not_confused_with_a_stranger() {
        let mut t = record_with_players(1, 4);
        t.generate_bracket();
        t.status = TournamentStatus::Active;

        // Semifinal 0 done: P0 beat P3. Semifinal 1 still in progress.
        {
            let m = t.matches[0].as_mut().unwrap();
            m.winner = Some("P0".into());
            m.status = MatchStatus::Completed;
        }
        // Advance P0 into the final, as record_result would.
        t.matches[2].as_mut().unwrap().player_white = Some("P0".into());

        let advanced = t.player_status("P0");
        assert_eq!(advanced.state, PlayerState::AwaitingOpponent);
        assert!(advanced.registered);
        let blocked = advanced.blocked_by.expect("should report what it waits on");
        assert_eq!(
            blocked.round, 0,
            "still blocked on the unfinished semifinal"
        );
        assert_eq!(blocked.matches_remaining, 1);
        let last = advanced.last_result.expect("should recall the win");
        assert!(last.won);
        assert_eq!(last.opponent, "P3");

        // The loser is eliminated, not waiting.
        assert_eq!(t.player_status("P3").state, PlayerState::Eliminated);

        // Someone who never registered is neither.
        let stranger = t.player_status("NOT_A_PLAYER");
        assert_eq!(stranger.state, PlayerState::NotRegistered);
        assert!(!stranger.registered);
        assert!(stranger.blocked_by.is_none());
    }

    #[test]
    fn match_ready_only_once_a_game_id_exists() {
        let mut t = record_with_players(2, 4);
        t.generate_bracket();
        t.status = TournamentStatus::Active;

        // Seated, both players known, but the scheduler hasn't stamped an id.
        assert_eq!(t.player_status("P0").state, PlayerState::AwaitingGameId);

        t.matches[0].as_mut().unwrap().game_id = Some(99);
        let ready = t.player_status("P0");
        assert_eq!(ready.state, PlayerState::MatchReady);
        assert_eq!(ready.r#match.expect("match present").game_id, Some(99));
    }

    #[test]
    fn registration_and_terminal_states() {
        let mut t = record_with_players(3, 4);
        assert_eq!(t.player_status("P0").state, PlayerState::Registered);

        t.generate_bracket();
        t.status = TournamentStatus::Completed;
        t.winner = Some("P1".into());
        t.second_place = Some("P0".into());
        t.prize_pool = 1_000_000;
        t.prize_shares = [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0];

        let champ = t.player_status("P1");
        assert_eq!(champ.state, PlayerState::Champion);
        assert_eq!(champ.placing, Some(1));
        assert_eq!(champ.prize_lamports, Some(600_000));

        let runner_up = t.player_status("P0");
        assert_eq!(runner_up.state, PlayerState::Eliminated);
        assert_eq!(runner_up.placing, Some(2));
        assert_eq!(runner_up.prize_lamports, Some(300_000));

        t.status = TournamentStatus::Cancelled;
        assert_eq!(t.player_status("P0").state, PlayerState::Cancelled);
    }

    #[test]
    fn sixteen_player_bracket_reports_round_progress() {
        let mut t = record_with_players(4, 16);
        t.generate_bracket();
        t.status = TournamentStatus::Active;
        assert_eq!(t.total_rounds(), 4);

        // Finish all 8 round-0 matches; P0 wins its own.
        for i in 0..8 {
            let m = t.matches[i].as_mut().unwrap();
            let w = m.player_white.clone().unwrap();
            m.winner = Some(w);
            m.status = MatchStatus::Completed;
        }
        assert_eq!(t.current_round(), Some(1));
        assert_eq!(t.matches_remaining_in_round(1), 4);

        // P0 advanced but its round-1 opponent isn't decided yet.
        t.matches[8].as_mut().unwrap().player_white = Some("P0".into());
        t.matches[8].as_mut().unwrap().player_black = None;
        let s = t.player_status("P0");
        assert_eq!(s.state, PlayerState::AwaitingOpponent);
        assert_eq!(s.round, Some(1));
        assert_eq!(s.total_rounds, 4);
    }
}
