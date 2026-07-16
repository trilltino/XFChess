//! Tournament data store for managing 8-128 player single-elimination tournaments.
//!
//! This module provides SQLite-backed storage for tournament records,
//! including player registration, bracket management, and match results.
//! Supports power-of-2 player counts: 8, 16, 32, 64, 128.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

/// Tournament format - single elimination or Swiss
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentFormat {
    SingleElimination,
    Swiss { rounds: u8 },
}

/// Swiss-specific storage data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwissStorageData {
    pub current_round: u8,
    pub total_rounds: u8,
    pub rounds: Vec<swiss_pairing::SwissRound>,
    pub results: Vec<(u8, u16, swiss_pairing::MatchResult)>,
    pub standings: Vec<swiss_pairing::StandingsEntry>,
    /// Unix timestamp (seconds) when the current round must end. None = no deadline set.
    #[serde(default)]
    pub round_deadline_at: Option<i64>,
    /// Player IDs marked absent for the current round
    #[serde(default)]
    pub absent_players: Vec<String>,
    /// Player IDs permanently withdrawn from the tournament
    #[serde(default)]
    pub withdrawn_players: Vec<String>,
    /// Player pairs that must not be matched
    #[serde(default)]
    pub forbidden_pairs: Vec<(String, String)>,
    /// Manual pairings to apply in the next round (cleared after start_round)
    #[serde(default)]
    pub manual_pairings_next_round: Vec<swiss_pairing::ManualPairing>,
}

/// Tournament lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentStatus {
    /// Registration phase - players can join
    Registration,
    /// Tournament in progress - matches being played
    Active,
    /// Tournament completed - winner determined
    Completed,
    /// Tournament cancelled
    Cancelled,
}

/// Individual match status within a tournament.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchStatus {
    /// Match not yet started
    Pending,
    /// Match currently in progress
    Active,
    /// Match completed
    Completed,
}

/// Source of match result determination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResultSource {
    /// Result recorded on-chain via Solana
    OnChain,
    /// Result submitted by backend oracle
    Oracle,
    /// Player forfeit (no-show)
    Forfeit,
    /// Players agreed to draw
    DrawAgreed,
}

/// Individual tournament match data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentMatch {
    /// Match index (0 to total_matches-1)
    pub match_index: u16,
    /// Round number (0 = first round, 1 = second round, etc.)
    pub round: u8,
    /// White player's wallet pubkey
    pub player_white: Option<String>,
    /// Black player's wallet pubkey
    pub player_black: Option<String>,
    /// Winner's wallet pubkey
    pub winner: Option<String>,
    /// Associated Solana game ID
    pub game_id: Option<u64>,
    /// Current match status
    pub status: MatchStatus,
    /// Source of result determination
    pub result_source: Option<ResultSource>,
    /// Next match index for the winner (None for final)
    pub next_match_for_winner: Option<u16>,
    /// Slot in next match (0 = white, 1 = black)
    pub next_match_slot: u8,
}

/// Complete tournament record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentRecord {
    /// Unique tournament identifier
    pub tournament_id: u64,
    /// Tournament display name
    pub name: String,
    /// Entry fee per player (in lamports)
    pub entry_fee_lamports: u64,
    /// Platform fee per player (in lamports)
    pub platform_fee_lamports: u64,
    /// Total prize pool (sum of entry fees)
    pub prize_pool: u64,
    /// Maximum players (8, 16, 32, 64, 128, 256)
    pub max_players: u16,
    /// Current tournament status
    pub status: TournamentStatus,
    /// Tournament format
    pub format: TournamentFormat,
    /// Registered player wallet pubkeys
    pub players: Vec<String>,
    /// Player ELO ratings (parallel to players vec)
    pub player_elos: Vec<u32>,
    /// Map from wallet pubkey to P2P node ID
    pub node_ids: HashMap<String, String>,
    /// All matches in the tournament (size = max_players - 1)
    pub matches: Vec<Option<TournamentMatch>>,
    /// Tournament winner (1st place)
    pub winner: Option<String>,
    /// Second place
    pub second_place: Option<String>,
    /// Third place
    pub third_place: Option<String>,
    /// Fourth place
    pub fourth_place: Option<String>,
    /// Fifth place
    pub fifth_place: Option<String>,
    /// Sixth place
    pub sixth_place: Option<String>,
    /// Seventh place
    pub seventh_place: Option<String>,
    /// Eighth place
    pub eighth_place: Option<String>,
    /// Ninth place
    pub ninth_place: Option<String>,
    /// Tenth place
    pub tenth_place: Option<String>,
    /// Prize distribution [1st-10th%] in basis points (10000 = 100%)
    pub prize_shares: [u16; 10],
    /// Swiss-specific data (None for single-elimination)
    pub swiss_data: Option<SwissStorageData>,
    /// Minimum ELO rating for players (optional)
    pub elo_min: Option<u32>,
    /// Maximum ELO rating for players (optional)
    pub elo_max: Option<u32>,
    /// Minimum players required to start tournament (optional)
    pub min_players: Option<u16>,
    /// Unix timestamp when tournament was created
    pub created_at: i64,
    /// Unix timestamp when tournament is scheduled to open for play (None = open immediately)
    pub scheduled_at: Option<i64>,
    /// Unix timestamp when tournament started
    pub started_at: Option<i64>,
    /// Unix timestamp when tournament completed
    pub completed_at: Option<i64>,
    /// Whether all entrants must have completed CACF KYC before joining
    #[serde(default)]
    pub kyc_required: bool,
    /// Optional bcrypt hash of join password (if private). When `Some`, `/join` must supply matching password.
    pub password_hash: Option<String>,
    /// True once the on-chain prize distribution crank has paid the winners.
    #[serde(default)]
    pub prizes_distributed: bool,
    /// Public spectator broadcast delay in seconds (0 = live). Stamped onto
    /// each match's game row so the public feed can't be used to ghost.
    #[serde(default)]
    pub broadcast_delay_secs: u32,
}

impl TournamentRecord {
    /// Creates a new tournament record.
    /// Default: 8 players, winner-take-all (10000 bps = 100%), single elimination
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
            broadcast_delay_secs: 0,
        }
    }

    /// Creates a tournament with custom configuration.
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
            broadcast_delay_secs: 0,
        }
    }

    /// Checks if the tournament is full.
    pub fn is_full(&self) -> bool {
        self.players.len() >= self.max_players as usize
    }

    /// Returns the index of the final match.
    pub fn final_match_index(&self) -> usize {
        self.matches.len() - 1
    }

    /// Returns the index of the first semifinal.
    /// For 8 players: match 4 (semifinal 1 of 2)
    /// For 16 players: match 12 (semifinal 1 of 2)
    pub fn semifinal1_index(&self) -> usize {
        self.final_match_index().saturating_sub(2)
    }

    /// Returns the index of the second semifinal.
    /// For 8 players: match 5 (semifinal 2 of 2)
    /// For 16 players: match 13 (semifinal 2 of 2)
    pub fn semifinal2_index(&self) -> usize {
        self.final_match_index().saturating_sub(1)
    }

    /// Finds the match assignment for a specific player.
    ///
    /// # Arguments
    /// * `player` - The player's wallet pubkey
    ///
    /// # Returns
    /// Match assignment if the player has an active match, None otherwise
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

    /// Swiss-aware match lookup for the player's current pairing or bye.
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

    /// Generates a complete single-elimination bracket.
    /// Call after tournament is started (players seeded by ELO).
    pub fn generate_bracket(&mut self) {
        let player_count = self.players.len();
        if player_count < 2 {
            self.matches.clear();
            return;
        }
        let total_matches = player_count - 1;

        // Initialize all matches
        self.matches = vec![None; total_matches];

        // Round 1: Pair seeded players (highest vs lowest)
        let round1_matches = player_count / 2;
        for i in 0..round1_matches {
            let white_idx = i;
            let black_idx = player_count - 1 - i;

            // In a 2-player bracket the single round-1 match IS the final.
            let next_match = if round1_matches == 1 {
                None
            } else {
                Some((round1_matches + i / 2) as u16)
            };
            let next_slot = if i % 2 == 0 { 0 } else { 1 };

            self.matches[i] = Some(TournamentMatch {
                match_index: i as u16,
                round: 0,
                player_white: Some(self.players[white_idx].clone()),
                player_black: Some(self.players[black_idx].clone()),
                winner: None,
                game_id: None,
                status: MatchStatus::Pending,
                result_source: None,
                next_match_for_winner: next_match,
                next_match_slot: next_slot,
            });
        }

        // Generate subsequent rounds
        let mut match_idx = round1_matches;
        let mut matches_in_round = round1_matches / 2;
        let mut round = 1u8;

        while matches_in_round > 0 {
            let round_start = match_idx;
            for i in 0..matches_in_round {
                let next_match = if matches_in_round == 1 {
                    None // Final match has no next match
                } else {
                    Some((round_start + matches_in_round + i / 2) as u16)
                };
                let next_slot = if i % 2 == 0 { 0 } else { 1 };

                self.matches[match_idx] = Some(TournamentMatch {
                    match_index: match_idx as u16,
                    round,
                    player_white: None, // Will be filled by winners
                    player_black: None,
                    winner: None,
                    game_id: None,
                    status: MatchStatus::Pending,
                    result_source: None,
                    next_match_for_winner: next_match,
                    next_match_slot: next_slot,
                });
                match_idx += 1;
            }
            matches_in_round /= 2;
            round += 1;
        }
    }

    /// Calculates prize payout for a given placement.
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

/// Match assignment result for a player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchAssignment {
    /// Match index (0 to total_matches-1)
    pub match_index: u16,
    /// Swiss round number when applicable.
    pub round: Option<u8>,
    /// Swiss board number when applicable.
    pub board: Option<u16>,
    /// Associated Solana game ID
    pub game_id: Option<u64>,
    /// Opponent's wallet pubkey
    pub opponent_pubkey: String,
    /// Opponent's P2P node ID
    pub opponent_node_id: Option<String>,
    /// Your color ("white" or "black")
    pub your_color: String,
    /// Current match status
    pub status: MatchStatus,
    /// True when the player received a Swiss bye this round.
    pub is_bye: bool,
}

/// SQLite-backed tournament store.
#[derive(Clone)]
pub struct TournamentStore {
    pool: SqlitePool,
}

impl TournamentStore {
    /// Creates a new TournamentStore with the provided pool.
    ///
    /// Creates the tournaments table if it doesn't exist.
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
        tracing::info!("[tournament-store] SQLite table ready");
        Self { pool }
    }

    /// Stores a tournament record.
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

    /// Retrieves a tournament by ID.
    pub async fn get(&self, id: u64) -> Option<TournamentRecord> {
        let row = sqlx::query("SELECT data FROM tournaments WHERE id = ?")
            .bind(id as i64)
            .fetch_optional(&self.pool)
            .await
            .ok()??;
        serde_json::from_str(&row.get::<String, _>(0)).ok()
    }

    /// Lists all tournaments.
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

    /// Updates a tournament with a closure.
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

    /// Registers a player's P2P node ID for the tournament.
    pub async fn register_node_id(&self, id: u64, player: String, node_id: String) -> bool {
        self.update(id, |t| {
            t.node_ids.insert(player, node_id);
        })
        .await
    }

    /// Removes a player from the tournament and decrements the prize pool.
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

    /// Sets the game ID for a specific match.
    pub async fn set_match_game_id(&self, id: u64, match_index: usize, game_id: u64) -> bool {
        self.update(id, |t| {
            if let Some(m) = t.matches[match_index].as_mut() {
                m.game_id = Some(game_id);
                m.status = MatchStatus::Active;
            }
        })
        .await
    }

    /// Records a match result and tracks placements for top 4.
    pub async fn record_result(
        &self,
        id: u64,
        match_index: usize,
        winner: String,
        loser: String,
    ) -> bool {
        self.update(id, |t| {
            if let Some(m) = t.matches[match_index].as_mut() {
                m.winner = Some(winner.clone());
                m.status = MatchStatus::Completed;
            }

            // Advance the winner into their next-round match slot (if any).
            let next = t.matches[match_index]
                .as_ref()
                .and_then(|m| m.next_match_for_winner.map(|n| (n as usize, m.next_match_slot)));
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
        .await
    }

    /// Update tournament status
    pub async fn update_status(&self, id: u64, status: TournamentStatus) -> bool {
        self.update(id, |t| {
            t.status = status;
        })
        .await
    }

    /// Seed players by ELO rating (highest to lowest)
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

    /// Generate bracket for single-elimination tournaments
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

    /// Start the tournament (generate bracket and set status)
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
