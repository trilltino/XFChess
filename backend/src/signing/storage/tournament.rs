//! Tournament data store for managing 8-128 player single-elimination tournaments.
//!
//! This module provides SQLite-backed storage for tournament records,
//! including player registration, bracket management, and match results.
//! Supports power-of-2 player counts: 8, 16, 32, 64, 128.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sqlx::{SqlitePool, Row};

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
    /// Total prize pool (sum of entry fees)
    pub prize_pool: u64,
    /// Maximum players (8, 16, 32, 64, 128)
    pub max_players: u16,
    /// Current tournament status
    pub status: TournamentStatus,
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
    /// Prize distribution [1st%, 2nd%, 3rd%, 4th%] in basis points (10000 = 100%)
    pub prize_shares: [u16; 4],
    /// Unix timestamp when tournament was created
    pub created_at: i64,
    /// Unix timestamp when tournament started
    pub started_at: Option<i64>,
    /// Unix timestamp when tournament completed
    pub completed_at: Option<i64>,
}

impl TournamentRecord {
    /// Creates a new tournament record.
    /// Default: 8 players, winner-take-all (10000 bps = 100%)
    pub fn new(tournament_id: u64, name: String, entry_fee_lamports: u64) -> Self {
        Self::with_config(tournament_id, name, entry_fee_lamports, 8, [10000, 0, 0, 0])
    }

    /// Creates a tournament with custom configuration.
    pub fn with_config(
        tournament_id: u64,
        name: String,
        entry_fee_lamports: u64,
        max_players: u16,
        prize_shares: [u16; 4],
    ) -> Self {
        let total_matches = (max_players - 1) as usize;
        Self {
            tournament_id,
            name,
            entry_fee_lamports,
            prize_pool: 0,
            max_players,
            status: TournamentStatus::Registration,
            players: Vec::with_capacity(max_players as usize),
            player_elos: Vec::with_capacity(max_players as usize),
            node_ids: HashMap::new(),
            matches: vec![None; total_matches],
            winner: None,
            second_place: None,
            third_place: None,
            fourth_place: None,
            prize_shares,
            created_at: chrono::Utc::now().timestamp(),
            started_at: None,
            completed_at: None,
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
                game_id: m.game_id,
                opponent_pubkey: opponent,
                opponent_node_id,
                your_color: if is_white { "white" } else { "black" }.to_string(),
                status: m.status.clone(),
            });
        }
        None
    }

    /// Generates a complete single-elimination bracket.
    /// Call after tournament is started (players seeded by ELO).
    pub fn generate_bracket(&mut self) {
        let player_count = self.max_players as usize;
        let total_matches = player_count - 1;

        // Initialize all matches
        self.matches = vec![None; total_matches];

        // Round 1: Pair seeded players (highest vs lowest)
        let round1_matches = player_count / 2;
        for i in 0..round1_matches {
            let white_idx = i;
            let black_idx = player_count - 1 - i;

            let next_match = if i % 2 == 0 {
                // Even index matches advance to next round's match i/2 as white
                Some((round1_matches + i / 2) as u16)
            } else {
                // Odd index matches advance to next round's match i/2 as black
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
                next_match_for_winner: next_match,
                next_match_slot: next_slot,
            });
        }

        // Generate subsequent rounds
        let mut match_idx = round1_matches;
        let mut matches_in_round = round1_matches / 2;
        let mut round = 1u8;

        while matches_in_round > 0 {
            for i in 0..matches_in_round {
                let next_match = if matches_in_round == 1 {
                    None // Final match has no next match
                } else if i % 2 == 0 {
                    Some((match_idx + matches_in_round + i / 2) as u16)
                } else {
                    Some((match_idx + matches_in_round + i / 2) as u16)
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
            )"
        ).execute(&pool).await.ok();
        tracing::info!("[tournament-store] SQLite table ready");
        Self { pool }
    }

    /// Stores a tournament record.
    pub async fn create(&self, record: TournamentRecord) {
        let data = serde_json::to_string(&record).unwrap_or_default();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT OR REPLACE INTO tournaments (id, data, updated_at) VALUES (?, ?, ?)"
        )
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
            .fetch_optional(&self.pool).await.ok()??;
        serde_json::from_str(&row.get::<String, _>(0)).ok()
    }

    /// Lists all tournaments.
    pub async fn list(&self) -> Vec<TournamentRecord> {
        let rows = sqlx::query("SELECT data FROM tournaments").fetch_all(&self.pool).await.unwrap();
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
                .execute(&self.pool).await.is_ok()
        } else {
            false
        }
    }

    /// Registers a player's P2P node ID for the tournament.
    pub async fn register_node_id(&self, id: u64, player: String, node_id: String) -> bool {
        self.update(id, |t| { t.node_ids.insert(player, node_id); }).await
    }

    /// Sets the game ID for a specific match.
    pub async fn set_match_game_id(&self, id: u64, match_index: usize, game_id: u64) -> bool {
        self.update(id, |t| {
            if let Some(m) = t.matches[match_index].as_mut() {
                m.game_id = Some(game_id);
                m.status = MatchStatus::Active;
            }
        }).await
    }

    /// Records a match result and tracks placements for top 4.
    pub async fn record_result(&self, id: u64, match_index: usize, winner: String, loser: String) -> bool {
        self.update(id, |t| {
            if let Some(m) = t.matches[match_index].as_mut() {
                m.winner = Some(winner.clone());
                m.status = MatchStatus::Completed;
            }

            let final_idx = t.final_match_index();
            let sf1_idx = t.semifinal1_index();
            let sf2_idx = t.semifinal2_index();

            if match_index == sf1_idx {
                // First semifinal - loser is 4th place
                t.fourth_place = Some(loser);
            } else if match_index == sf2_idx {
                // Second semifinal - loser is 3rd place
                t.third_place = Some(loser);
            } else if match_index == final_idx {
                // Final complete - tournament done
                t.winner = Some(winner);
                t.second_place = Some(loser);
                t.status = TournamentStatus::Completed;
                t.completed_at = Some(chrono::Utc::now().timestamp());
            }
        }).await
    }
}
