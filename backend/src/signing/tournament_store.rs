use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentMatch {
    pub match_index: u8,
    pub round: u8,
    pub player_white: Option<String>,
    pub player_black: Option<String>,
    pub winner: Option<String>,
    pub game_id: Option<u64>,
    pub status: MatchStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentRecord {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub status: TournamentStatus,
    pub players: Vec<String>,
    pub player_elos: Vec<u32>,
    pub node_ids: HashMap<String, String>,
    pub matches: [Option<TournamentMatch>; 3],
    pub winner: Option<String>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

impl TournamentRecord {
    pub fn new(tournament_id: u64, name: String, entry_fee_lamports: u64) -> Self {
        Self {
            tournament_id,
            name,
            entry_fee_lamports,
            prize_pool: 0,
            status: TournamentStatus::Registration,
            players: Vec::new(),
            player_elos: Vec::new(),
            node_ids: HashMap::new(),
            matches: [None, None, None],
            winner: None,
            created_at: chrono::Utc::now().timestamp(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn is_full(&self) -> bool {
        self.players.len() >= 4
    }

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
    pub match_index: u8,
    pub game_id: Option<u64>,
    pub opponent_pubkey: String,
    pub opponent_node_id: Option<String>,
    pub your_color: String,
    pub status: MatchStatus,
}

#[derive(Clone)]
pub struct TournamentStore {
    inner: Arc<RwLock<HashMap<u64, TournamentRecord>>>,
}

impl Default for TournamentStore {
    fn default() -> Self {
        Self { inner: Arc::new(RwLock::new(HashMap::new())) }
    }
}

impl TournamentStore {
    pub fn new() -> Self { Self::default() }

    pub async fn create(&self, record: TournamentRecord) {
        self.inner.write().await.insert(record.tournament_id, record);
    }

    pub async fn get(&self, id: u64) -> Option<TournamentRecord> {
        self.inner.read().await.get(&id).cloned()
    }

    pub async fn list(&self) -> Vec<TournamentRecord> {
        self.inner.read().await.values().cloned().collect()
    }

    pub async fn update<F: FnOnce(&mut TournamentRecord)>(&self, id: u64, f: F) -> bool {
        let mut map = self.inner.write().await;
        if let Some(r) = map.get_mut(&id) { f(r); true } else { false }
    }

    pub async fn register_node_id(&self, id: u64, player: String, node_id: String) -> bool {
        self.update(id, |t| { t.node_ids.insert(player, node_id); }).await
    }

    pub async fn set_match_game_id(&self, id: u64, match_index: usize, game_id: u64) -> bool {
        self.update(id, |t| {
            if let Some(m) = t.matches[match_index].as_mut() {
                m.game_id = Some(game_id);
                m.status = MatchStatus::Active;
            }
        }).await
    }

    pub async fn record_result(&self, id: u64, match_index: usize, winner: String) -> bool {
        self.update(id, |t| {
            if let Some(m) = t.matches[match_index].as_mut() {
                m.winner = Some(winner.clone());
                m.status = MatchStatus::Completed;
            }
            if match_index < 2 {
                let sf1_done = t.matches[0].as_ref().map(|m| m.status == MatchStatus::Completed).unwrap_or(false);
                let sf2_done = t.matches[1].as_ref().map(|m| m.status == MatchStatus::Completed).unwrap_or(false);
                if sf1_done && sf2_done {
                    let w1 = t.matches[0].as_ref().and_then(|m| m.winner.clone());
                    let w2 = t.matches[1].as_ref().and_then(|m| m.winner.clone());
                    if let (Some(w1), Some(w2)) = (w1, w2) {
                        t.matches[2] = Some(TournamentMatch {
                            match_index: 2, round: 1,
                            player_white: Some(w1), player_black: Some(w2),
                            winner: None, game_id: None, status: MatchStatus::Pending,
                        });
                    }
                }
            } else {
                t.winner = Some(winner);
                t.status = TournamentStatus::Completed;
                t.completed_at = Some(chrono::Utc::now().timestamp());
            }
        }).await
    }
}
