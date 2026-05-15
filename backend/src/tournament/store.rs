use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Default maximum ELO rating for tournaments
pub const DEFAULT_ELO_MAX: u32 = 9999;

/// Default minimum ELO rating for tournaments
pub const DEFAULT_ELO_MIN: u32 = 0;

/// Default minimum players for tournaments
pub const DEFAULT_MIN_PLAYERS: u16 = 8;

// ── Tournament Format ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentType {
    SingleElimination,
    Swiss { rounds: u8 },
}

// ── Domain types ──────────────────────────────────────────────────────────────

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
    pub round: u16,
    pub player_white: Option<String>,
    pub player_black: Option<String>,
    pub winner: Option<String>,
    pub game_id: Option<u64>,
    pub status: MatchStatus,
    pub result_source: Option<ResultSource>,
}

// Swiss-specific state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwissState {
    pub current_round: u8,
    pub total_rounds: u8,
    pub player_scores: HashMap<String, f64>,
    pub player_byes: HashMap<String, u8>,
    pub pairings_history: Vec<Vec<(String, String)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentRecord {
    pub tournament_id: u64,
    pub name: String,
    pub tournament_type: TournamentType,
    pub max_players: u16,
    pub min_players: u16,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub prize_mint: Option<String>,
    pub status: TournamentStatus,
    pub players: Vec<String>,
    pub player_elos: Vec<u32>,
    pub node_ids: HashMap<String, String>,
    pub matches: Vec<TournamentMatch>,
    pub swiss_state: Option<SwissState>,
    pub winner: Option<String>,
    pub scheduled_at: Option<i64>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub created_at: i64,
    pub elo_min: u32,
    pub elo_max: u32,
}

impl TournamentRecord {
    pub fn new(
        tournament_id: u64,
        name: String,
        entry_fee_lamports: u64,
        tournament_type: TournamentType,
        max_players: u16,
    ) -> Self {
        Self {
            tournament_id,
            name,
            tournament_type,
            max_players,
            min_players: max_players.min(8),
            entry_fee_lamports,
            prize_pool: 0,
            prize_mint: None,
            status: TournamentStatus::Registration,
            players: Vec::new(),
            player_elos: Vec::new(),
            node_ids: HashMap::new(),
            matches: Vec::new(),
            swiss_state: None,
            winner: None,
            scheduled_at: None,
            created_at: chrono::Utc::now().timestamp(),
            started_at: None,
            completed_at: None,
            elo_min: DEFAULT_ELO_MIN,
            elo_max: DEFAULT_ELO_MAX,
        }
    }

    pub fn with_config(
        tournament_id: u64,
        name: String,
        entry_fee_lamports: u64,
        max_players: u16,
        prize_shares: [u16; 4],
        format: TournamentType,
        elo_min: Option<u32>,
        elo_max: Option<u32>,
        min_players: Option<u16>,DEFAULT_MIN_PLAYERS
        scheduled_at: Option<i64>,
        _kyc_required: bool,
    ) -> Self {
        Self {
            tournament_id,
            name,
            tournament_type: format,
            max_players,
            min_players: min_players.unwrap_or(8),
            entry_fee_lamports,
            prize_pool: 0,
            prize_mint: None,
            status: TournamentStatus::Registration,
            players: Vec::new(),
            player_elos: Vec::new(),
            node_ids: HashMap::new(),
            matches: Vec::new(),
            swiss_state: None,
            winner: None,
            scheduled_at,
            created_at: chrono::Utc::now().timestamp(),
            started_at: None,
            completed_at: None,
            elo_min: elo_min.unwrap_or(DEFAULT_ELO_MIN),
            elo_max: elo_max.unwrap_or(DEFAULT_ELO_MAX),
        }
    }

    pub fn is_full(&self) -> bool {
        self.players.len() >= self.max_players as usize
    }

    pub fn final_match_index(&self) -> usize {
        match self.tournament_type {
            TournamentType::SingleElimination => self.max_players.saturating_sub(1) as usize,
            TournamentType::Swiss { rounds } => rounds as usize,
        }
    }

    /// Returns the match assigned to the given player pubkey, if any.
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchAssignment {
    pub match_index: u16,
    pub game_id: Option<u64>,
    pub opponent_pubkey: String,
    pub opponent_node_id: Option<String>,
    pub your_color: String,
    pub status: MatchStatus,
}

// ── Store ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TournamentStore {
    inner: Arc<RwLock<HashMap<u64, TournamentRecord>>>,
}

impl Default for TournamentStore {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl TournamentStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create(&self, record: TournamentRecord) {
        let mut map = self.inner.write().await;
        map.insert(record.tournament_id, record);
    }

    pub async fn get(&self, id: u64) -> Option<TournamentRecord> {
        self.inner.read().await.get(&id).cloned()
    }

    pub async fn list(&self) -> Vec<TournamentRecord> {
        self.inner.read().await.values().cloned().collect()
    }

    pub async fn update<F>(&self, id: u64, f: F) -> bool
    where
        F: FnOnce(&mut TournamentRecord),
    {
        let mut map = self.inner.write().await;
        if let Some(record) = map.get_mut(&id) {
            f(record);
            true
        } else {
            false
        }
    }

    /// Register a player's iroh node ID.
    pub async fn register_node_id(&self, tournament_id: u64, player: String, node_id: String) -> bool {
        self.update(tournament_id, |t| {
            t.node_ids.insert(player, node_id);
        })
        .await
    }

    /// Set a game_id on a match slot (called when backend creates the Game PDA).
    pub async fn set_match_game_id(&self, tournament_id: u64, match_index: usize, game_id: u64) -> bool {
        self.update(tournament_id, |t| {
            if let Some(m) = t.matches[match_index].as_mut() {
                m.game_id = Some(game_id);
                m.status = MatchStatus::Active;
            }
        })
        .await
    }

    /// Record match result and optionally advance to final.
    pub async fn record_result(
        &self,
        tournament_id: u64,
        match_index: usize,
        winner: String,
    ) -> bool {
        self.update(tournament_id, |t| {
            if let Some(m) = t.matches[match_index].as_mut() {
            }

            // If both semi-finals are done, populate the final
            let sf1_done = t.matches.get(0).and_then(|m| m.as_ref()).map_or(false, |m| m.status == MatchStatus::Completed);
            let sf2_done = t.matches.get(1).and_then(|m| m.as_ref()).map_or(false, |m| m.status == MatchStatus::Completed);

            if sf1_done && sf2_done {
                let sf1_winner = t.matches.get(0).and_then(|m| m.as_ref()).and_then(|m| m.winner.clone());
                let sf2_winner = t.matches.get(1).and_then(|m| m.as_ref()).and_then(|m| m.winner.clone());

                if let (Some(w1), Some(w2)) = (sf1_winner, sf2_winner) {
                    if let Some(final_slot) = t.matches.get_mut(2) {
                        *final_slot = Some(TournamentMatch {
                            match_index: 2,
                            round: 1,
                            player_white: Some(w1),
                            player_black: Some(w2),
                            winner: None,
                            game_id: None,
                            status: MatchStatus::Pending,
                        });
                    }
                }
            } else {
                // Final completed
                t.winner = Some(winner);
                t.status = TournamentStatus::Completed;
                t.completed_at = Some(chrono::Utc::now().timestamp());
            }
        })
        .await
    }
}
