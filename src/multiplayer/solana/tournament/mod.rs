//! Client side of tournaments: discovery and status polling ([`polling`]),
//! registration ([`registration`]), live Swiss updates ([`swiss`]) and starting
//! assigned matches ([`matches`]).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};

mod matches;
mod polling;
mod registration;
mod swiss;
use matches::*;
use polling::*;
pub use registration::*;
pub use swiss::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentStatus {
    Registration,
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TournamentFormat {
    SingleElimination,
    Swiss { rounds: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub player_count: u16,
    pub max_players: u16,
    pub status: TournamentStatus,
    pub format: TournamentFormat,
    pub started_at: Option<i64>,
    pub scheduled_at: Option<i64>,
    pub is_private: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GameTab {
    #[default]
    Live,
    Upcoming,
    Finished,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum TournamentJoinStatus {
    #[default]
    Idle,
    Pending,
    Registered(usize),
    Error(String),
}

#[derive(Resource)]
pub struct TournamentClientState {
    pub active_tournament_id: Option<u64>,
    pub my_slot: Option<usize>,
    pub available_tournaments: Vec<crate::multiplayer::network::vps::TournamentSummary>,
    pub status_message: String,
    pub poll_timer: f32,
    pub join_status: TournamentJoinStatus,
    pub tx_rx: Option<oneshot::Receiver<Result<usize, String>>>,
    pub list_rx: Option<
        crossbeam_channel::Receiver<
            Result<Vec<crate::multiplayer::network::vps::TournamentSummary>, String>,
        >,
    >,
    pub last_list_poll: Option<Instant>,
    pub tournament_games: Vec<crate::multiplayer::network::vps::TournamentGameListing>,
    pub games_rx: Option<
        crossbeam_channel::Receiver<Vec<crate::multiplayer::network::vps::TournamentGameListing>>,
    >,
    pub last_games_poll: Option<Instant>,
    pub detail_games: Vec<crate::multiplayer::network::vps::TournamentGameListing>,
    pub detail_games_for: Option<u64>,
    pub detail_games_rx: Option<
        crossbeam_channel::Receiver<
            Result<Vec<crate::multiplayer::network::vps::TournamentGameListing>, String>,
        >,
    >,
    pub last_detail_poll: Option<Instant>,
    pub detail_tab: GameTab,
    pub bracket_fired_rx: Option<crossbeam_channel::Receiver<BracketFiredEvent>>,
    pub bracket_ready: bool,
    pub password_input: String,
    pub password_error: Option<String>,
    pub last_poll_error: Option<String>,
    pub waiting_for_next_match: bool,
    pub last_match_result: Option<String>,
    pub active_game_id: Option<u64>,
    pub status_filter: Option<String>,
    pub expanded_tournament_id: Option<u64>,

    // Waiting room
    pub registered_players: Vec<String>,
    pub players_rx: Option<crossbeam_channel::Receiver<Vec<String>>>,
    pub players_poll_timer: f32,
    pub private_code_input: String,
    pub private_code_error: Option<String>,
    pub private_join_rx: Option<crossbeam_channel::Receiver<Result<(), String>>>,
    pub leave_rx: Option<crossbeam_channel::Receiver<Result<String, String>>>,
    pub money_flow: crate::multiplayer::solana::money_flow::MoneyFlowState,

    // Tournament waiting-room chat
    pub chat_messages: Vec<(String, String)>,
    pub chat_rx: Option<crossbeam_channel::Receiver<(String, String)>>,
    pub chat_input: String,
    pub chat_tx: Option<crossbeam_channel::Sender<(String, String)>>,
    pub chat_stop_tx: Option<crossbeam_channel::Sender<()>>,

    pub status_rx: Option<
        crossbeam_channel::Receiver<
            Result<crate::multiplayer::network::vps::MyTournamentStatus, String>,
        >,
    >,
    pub my_state: Option<crate::multiplayer::network::vps::PlayerState>,
    pub placing: Option<u8>,
    pub prize_lamports: Option<u64>,

    pub dismissed_ids: std::collections::HashSet<u64>,
}

impl Default for TournamentClientState {
    fn default() -> Self {
        Self {
            active_tournament_id: None,
            my_slot: None,
            available_tournaments: Vec::new(),
            status_message: String::new(),
            poll_timer: 0.0,
            join_status: TournamentJoinStatus::Idle,
            tx_rx: None,
            list_rx: None,
            last_list_poll: None,
            tournament_games: Vec::new(),
            games_rx: None,
            last_games_poll: None,
            detail_games: Vec::new(),
            detail_games_for: None,
            detail_games_rx: None,
            last_detail_poll: None,
            detail_tab: GameTab::Live,
            bracket_fired_rx: None,
            bracket_ready: false,
            password_input: String::new(),
            password_error: None,
            last_poll_error: None,
            waiting_for_next_match: false,
            last_match_result: None,
            active_game_id: None,
            status_filter: None,
            expanded_tournament_id: None,
            registered_players: Vec::new(),
            players_rx: None,
            players_poll_timer: 0.0,
            private_code_input: String::new(),
            private_code_error: None,
            private_join_rx: None,
            leave_rx: None,
            money_flow: crate::multiplayer::solana::money_flow::MoneyFlowState::default(),
            chat_messages: Vec::new(),
            chat_rx: None,
            chat_input: String::new(),
            chat_tx: None,
            chat_stop_tx: None,
            dismissed_ids: std::collections::HashSet::new(),
            status_rx: None,
            my_state: None,
            placing: None,
            prize_lamports: None,
        }
    }
}

impl TournamentClientState {
    pub fn reset(&mut self) {
        self.stop_chat();
        *self = Self::default();
    }

    pub fn stop_chat(&mut self) {
        if let Some(tx) = self.chat_stop_tx.take() {
            let _ = tx.send(());
        }
        self.chat_rx = None;
        self.chat_tx = None;
        self.chat_messages.clear();
        self.chat_input.clear();
    }

    pub fn start_chat(&mut self, tournament_id: u64, player_name: String) {
        if self.chat_rx.is_some() {
            return;
        }
        let (inbound_tx, inbound_rx) = crossbeam_channel::bounded::<(String, String)>(64);
        let (outbound_tx, outbound_rx) = crossbeam_channel::bounded::<(String, String)>(16);
        let (stop_tx, stop_rx) = crossbeam_channel::bounded::<()>(1);
        self.chat_rx = Some(inbound_rx);
        self.chat_tx = Some(outbound_tx);
        self.chat_stop_tx = Some(stop_tx);
        let base = crate::multiplayer::network::vps::vps_base();
        // Background thread: poll for new messages every 3 seconds
        let base2 = base.clone();
        let itx = inbound_tx.clone();
        crate::core::net_task::spawn_detached("tournament", move || {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new());
            let mut last_id: u64 = 0;
            loop {
                match stop_rx.try_recv() {
                    Ok(_) | Err(crossbeam_channel::TryRecvError::Disconnected) => break,
                    Err(crossbeam_channel::TryRecvError::Empty) => {}
                }
                let url = format!(
                    "{}/api/tournament/{}/chat?after={}",
                    base2, tournament_id, last_id
                );
                if let Ok(resp) = client.get(&url).send() {
                    if let Ok(msgs) = resp.json::<Vec<serde_json::Value>>() {
                        for msg in msgs {
                            let id = msg.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                            let sender = msg
                                .get("player")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?")
                                .to_string();
                            let text = msg
                                .get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if id > last_id {
                                last_id = id;
                            }
                            if itx.send((sender, text)).is_err() {
                                return;
                            }
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        });
        // Background thread: send outbound messages
        crate::core::net_task::spawn_detached("tournament", move || {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new());
            while let Ok((player, text)) = outbound_rx.recv() {
                let url = format!("{}/api/tournament/{}/chat", base, tournament_id);
                let body = serde_json::json!({ "player": player, "text": text });
                let _ = client.post(&url).json(&body).send();
                let _ = player; // suppress move warning
            }
        });
        let _ = player_name;
    }

    pub fn drain_chat(&mut self) {
        if let Some(ref rx) = self.chat_rx {
            while let Ok(msg) = rx.try_recv() {
                self.chat_messages.push(msg);
                if self.chat_messages.len() > 200 {
                    self.chat_messages.remove(0);
                }
            }
        }
    }

    pub fn is_registered(&self) -> bool {
        matches!(self.join_status, TournamentJoinStatus::Registered(_))
    }
}

#[derive(Message, Debug, Clone)]
pub struct OpenTournamentDiscoveryEvent;

#[derive(Message, Debug, Clone)]
pub struct RegisterForTournamentEvent {
    pub tournament_id: u64,
    pub password: Option<String>,
}

#[derive(Message, Debug, Clone)]
pub struct TournamentMatchAssignedEvent {
    pub tournament_id: u64,
    pub match_index: u8,
    pub game_id: Option<u64>,
    pub opponent_pubkey: String,
    pub opponent_node_id: Option<String>,
    pub your_color: String,
}

#[derive(Message, Debug, Clone)]
pub struct SwissRoundStartedEvent {
    pub tournament_id: u64,
    pub round: u8,
    pub pairings: Vec<(String, String)>,
}

#[derive(Message, Debug, Clone)]
pub struct SwissResultRecordedEvent {
    pub tournament_id: u64,
    pub round: u8,
    pub board: u16,
    pub white: String,
    pub black: String,
    pub result: String,
}

#[derive(Message, Debug, Clone)]
pub struct SwissStandingsUpdatedEvent {
    pub tournament_id: u64,
    pub standings: Vec<(String, f64, u16)>,
}

#[derive(Message, Debug, Clone)]
pub struct BracketFiredEvent {
    pub tournament_id: u64,
    pub player_count: u16,
    pub started_at: i64,
}

pub struct TournamentClientPlugin;

impl Plugin for TournamentClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TournamentClientState>()
            .add_message::<TournamentMatchAssignedEvent>()
            .add_systems(
                Update,
                (
                    poll_tournament_tasks,
                    poll_tournament_list,
                    poll_tournament_games_list,
                    poll_expanded_tournament_games,
                    poll_bracket_fired,
                    poll_my_tournament_status,
                    handle_tournament_match_assigned,
                )
                    .chain()
                    .in_set(crate::core::schedule::SessionSet::Tournament),
            );
    }
}
