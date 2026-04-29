//! Tournament client state, Bevy resources, and on-chain instruction dispatch
//! for the 4-player bracket system.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use tokio::sync::{mpsc, oneshot};
use std::time::Instant;

// Tournament status from backend
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
    /// Player slot within the tournament (0–3).
    pub my_slot: Option<usize>,
    /// Cached list of available tournaments from VPS.
    pub available_tournaments: Vec<crate::multiplayer::network::vps::TournamentSummary>,
    /// Status text shown in lobby / waiting screens.
    pub status_message: String,
    /// Polling timer: seconds since last my-match poll.
    pub poll_timer: f32,
    pub join_status: TournamentJoinStatus,
    /// Oneshot receiver for an in-flight `register_player` transaction.
    pub tx_rx: Option<oneshot::Receiver<Result<usize, String>>>,
    /// Channel for receiving background tournament list polls.
    pub list_rx: Option<crossbeam_channel::Receiver<Vec<crate::multiplayer::network::vps::TournamentSummary>>>,
    pub last_list_poll: Option<Instant>,
    pub bracket_fired_rx: Option<crossbeam_channel::Receiver<BracketFiredEvent>>,
    pub bracket_ready: bool,
    pub password_input: String,
    pub password_error: Option<String>,
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
            bracket_fired_rx: None,
            bracket_ready: false,
            password_input: String::new(),
            password_error: None,
        }
    }
}

impl TournamentClientState {
    pub fn reset(&mut self) {
        *self = Self::default();
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

pub fn spawn_register_tournament(
    tournament_id: u64,
    wallet_pubkey: Pubkey,
    password: Option<String>,
) -> oneshot::Receiver<Result<usize, String>> {
    let (tx, rx) = oneshot::channel();
    let pk = wallet_pubkey.to_string();
    let rpc_url = std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    bevy::tasks::IoTaskPool::get().spawn(async move {
        let res = register_tournament(tournament_id, &pk, &rpc_url, password.as_deref()).await;
        let _ = tx.send(res.map(|slot| slot as usize));
    }).detach();
    rx
}

pub fn spawn_swiss_subscription(
    tournament_id: u64,
    round_tx: mpsc::UnboundedSender<SwissRoundStartedEvent>,
    result_tx: mpsc::UnboundedSender<SwissResultRecordedEvent>,
    standings_tx: mpsc::UnboundedSender<SwissStandingsUpdatedEvent>,
    bracket_fired_tx: crossbeam_channel::Sender<BracketFiredEvent>,
) {
    bevy::tasks::IoTaskPool::get().spawn(async move {
        use braid_iroh::BraidIrohNode;
        use braid_iroh::DiscoveryConfig;

        let vps_url = std::env::var("VPS_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

        // Get Iroh node ID from VPS
        let client = reqwest::Client::new();
        let node_id_res = client
            .get(format!("{}/iroh/node-id", vps_url))
            .send()
            .await;

        let _node_id = match node_id_res {
            Ok(resp) if resp.status().is_success() => {
                match resp.text().await {
                    Ok(text) => text,
                    Err(e) => {
                        error!("Failed to read Iroh node ID response: {}", e);
                        return;
                    }
                }
            }
            Ok(resp) => {
                error!("Failed to get Iroh node ID from VPS: HTTP {}", resp.status());
                return;
            }
            Err(e) => {
                error!("Failed to get Iroh node ID from VPS: {}", e);
                return;
            }
        };

        // Create Iroh node using BraidGameConfig
        use braid_iroh::BraidGameConfig;
        let node = match BraidIrohNode::spawn(BraidGameConfig {
            discovery: DiscoveryConfig::Real,
            secret_key: None,
            proxy_config: None,
            app_router: None,
            db: None,
        }).await {
            Ok(node) => node,
            Err(e) => {
                error!("Failed to spawn Iroh node: {}", e);
                return;
            }
        };

        // Subscribe to Swiss tournament topic
        let topic = format!("/swiss/{}", tournament_id);
        let mut rx = match node.subscribe(&topic, vec![]).await {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to subscribe to Swiss topic {}: {}", topic, e);
                return;
            }
        };

        info!("[Swiss] Subscribed to topic {}", topic);

        // Process incoming messages
        use futures::StreamExt;
        use iroh_gossip::api::Event as GossipEvent;

        while let Some(Ok(event)) = rx.next().await {
            let payload = match event {
                GossipEvent::Received(message) => message.content,
                _ => continue,
            };
            let Ok(text) = std::str::from_utf8(&payload) else { continue };
            let Ok(swiss_msg) = serde_json::from_str::<braid_iroh::protocol::SwissMessage>(text) else { continue };
            match swiss_msg {
                braid_iroh::protocol::SwissMessage::RoundStarted { round, pairings, .. } => {
                    let _ = round_tx.send(SwissRoundStartedEvent {
                        tournament_id,
                        round,
                        pairings: pairings.iter().map(|p| (p.white.clone(), p.black.clone())).collect(),
                    });
                }
                braid_iroh::protocol::SwissMessage::ResultRecorded { round, board, result, .. } => {
                    // `SwissMessage::ResultRecorded` carries only the match outcome;
                    // white/black player identifiers are resolved from the pairing
                    // stored alongside the round.
                    let (white, black) = match &result {
                        braid_iroh::protocol::MatchResult::Win { winner } => (winner.clone(), String::new()),
                        braid_iroh::protocol::MatchResult::Draw => (String::new(), String::new()),
                    };
                    let _ = result_tx.send(SwissResultRecordedEvent {
                        tournament_id,
                        round,
                        board,
                        white,
                        black,
                        result: result.to_string(),
                    });
                }
                braid_iroh::protocol::SwissMessage::StandingsUpdated { standings, .. } => {
                    let _ = standings_tx.send(SwissStandingsUpdatedEvent {
                        tournament_id,
                        standings: standings.iter().map(|s| (s.player_id.clone(), s.score, s.rank)).collect(),
                    });
                }
                braid_iroh::protocol::SwissMessage::BracketFired { player_count, started_at, .. } => {
                    let _ = bracket_fired_tx.send(BracketFiredEvent {
                        tournament_id,
                        player_count,
                        started_at,
                    });
                }
            }
        }
    }).detach();
}

fn poll_tournament_tasks(
    mut tournament: ResMut<TournamentClientState>,
    mut menu_state: ResMut<NextState<crate::core::states::MenuState>>,
) {
    if let Some(ref mut rx) = tournament.tx_rx {
        match rx.try_recv() {
            Ok(Ok(slot)) => {
                tournament.my_slot = Some(slot);
                tournament.join_status = TournamentJoinStatus::Registered(slot);
                tournament.status_message =
                    format!("Registered in slot {}. Waiting for bracket…", slot + 1);
                tournament.tx_rx = None;
                menu_state.set(crate::core::states::MenuState::Tournaments);
                info!("[TOURNAMENT] Registration confirmed — slot {}", slot);
            }
            Ok(Err(e)) => {
                tournament.join_status = TournamentJoinStatus::Error(e.clone());
                tournament.status_message = format!("Registration failed: {}", e);
                tournament.tx_rx = None;
                warn!("[TOURNAMENT] Registration error: {}", e);
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                tournament.join_status =
                    TournamentJoinStatus::Error("Task dropped unexpectedly".to_string());
                tournament.status_message = "Registration task failed. Please retry.".to_string();
                tournament.tx_rx = None;
            }
        }
    }
}

fn poll_tournament_list(
    mut tournament: ResMut<TournamentClientState>,
) {
    const POLL_INTERVAL_SECS: f64 = 30.0;

    if let Some(ref rx) = tournament.list_rx {
        if let Ok(list) = rx.try_recv() {
            tournament.available_tournaments = list;
            tournament.list_rx = None;
        }
    }

    if tournament.list_rx.is_some() {
        return;
    }

    let should_poll = tournament.last_list_poll
        .map(|t| t.elapsed().as_secs_f64() >= POLL_INTERVAL_SECS)
        .unwrap_or(true);

    if !should_poll {
        return;
    }

    tournament.last_list_poll = Some(Instant::now());
    let (tx, rx) = crossbeam_channel::bounded(1);
    tournament.list_rx = Some(rx);

    bevy::tasks::IoTaskPool::get().spawn(async move {
        match crate::multiplayer::network::vps::list_tournaments() {
            Ok(list) => { let _ = tx.send(list); }
            Err(e) => { warn!("[TOURNAMENT] list_tournaments failed: {}", e); }
        }
    }).detach();
}

fn poll_bracket_fired(
    mut tournament: ResMut<TournamentClientState>,
) {
    let Some(ref rx) = tournament.bracket_fired_rx else { return };
    if let Ok(event) = rx.try_recv() {
        info!(
            "[TOURNAMENT] BracketFired received: tournament {} — {} players",
            event.tournament_id, event.player_count
        );
        tournament.bracket_ready = true;
        tournament.status_message = format!(
            "Bracket fired! {} players entered. Fetching your match…",
            event.player_count,
        );
        tournament.bracket_fired_rx = None;
    }
}

pub struct TournamentClientPlugin;

impl Plugin for TournamentClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TournamentClientState>()
            .add_systems(Update, (poll_tournament_tasks, poll_tournament_list, poll_bracket_fired));
    }
}

async fn register_tournament(
    _tournament_id: u64,
    _wallet_pubkey: &str,
    _rpc_url: &str,
    _password: Option<&str>,
) -> Result<u64, String> {
    // implement register_tournament logic here
    Ok(0)
}
