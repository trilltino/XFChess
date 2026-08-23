//! Tournament client state, Bevy resources, and on-chain instruction dispatch
//! for the 4-player bracket system.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};

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

/// Tabs in a tournament card: the games happening now, the ones still to
/// come, and the ones you can replay.
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
    /// Channel for receiving background tournament list polls. Carries the
    /// real fetch/parse error on failure — this used to be dropped in favor
    /// of a generic "channel closed" message, which made a real backend
    /// outage or a schema mismatch indistinguishable from a genuinely empty
    /// tournament list.
    pub list_rx: Option<
        crossbeam_channel::Receiver<
            Result<Vec<crate::multiplayer::network::vps::TournamentSummary>, String>,
        >,
    >,
    pub last_list_poll: Option<Instant>,
    /// On-chain games created by the backend orchestrator across all
    /// tournaments (bracket matches with a Solana game_id) — feeds the
    /// per-tournament "Watch Live" list in the browser.
    pub tournament_games: Vec<crate::multiplayer::network::vps::TournamentGameListing>,
    /// Receiver for the background tournament-games fetch.
    pub games_rx: Option<
        crossbeam_channel::Receiver<Vec<crate::multiplayer::network::vps::TournamentGameListing>>,
    >,
    pub last_games_poll: Option<Instant>,
    /// Games of the tournament currently expanded in the browser, from
    /// `GET /api/tournament/{id}/games`.
    ///
    /// Separate from `tournament_games` (the cross-tournament walk) because
    /// this one is *rich* — backend-computed watchability, move counts,
    /// results — and scoped to one tournament, so it can poll fast without the
    /// per-tournament bracket fan-out.
    pub detail_games: Vec<crate::multiplayer::network::vps::TournamentGameListing>,
    /// Which tournament `detail_games` describes.
    pub detail_games_for: Option<u64>,
    pub detail_games_rx: Option<
        crossbeam_channel::Receiver<
            Result<Vec<crate::multiplayer::network::vps::TournamentGameListing>, String>,
        >,
    >,
    pub last_detail_poll: Option<Instant>,
    /// Which of the Live / Upcoming / Finished tabs the expanded card shows.
    pub detail_tab: GameTab,
    pub bracket_fired_rx: Option<crossbeam_channel::Receiver<BracketFiredEvent>>,
    pub bracket_ready: bool,
    pub password_input: String,
    pub password_error: Option<String>,
    /// Last error from tournament list poll (cleared on success).
    pub last_poll_error: Option<String>,
    /// Set when the player finished a match and is waiting for the next round assignment.
    pub waiting_for_next_match: bool,
    /// Result of the last completed match (e.g. "1-0", "0-1", "½-½").
    pub last_match_result: Option<String>,
    /// Active on-chain game for the current assigned tournament match.
    pub active_game_id: Option<u64>,
    /// Filter shown in the tournament browser (None = All).
    pub status_filter: Option<String>,
    /// Which tournament card is currently expanded to show details.
    pub expanded_tournament_id: Option<u64>,

    // Waiting room
    /// Registered players fetched from GET /api/tournament/{id}/players.
    pub registered_players: Vec<String>,
    /// Receiver for the players poll task.
    pub players_rx: Option<crossbeam_channel::Receiver<Vec<String>>>,
    /// Timer for polling player list (reset every 5 s).
    pub players_poll_timer: f32,
    /// "Enter invite code" text input.
    pub private_code_input: String,
    /// Error from private join attempt.
    pub private_code_error: Option<String>,
    /// Receiver for private join response.
    pub private_join_rx: Option<crossbeam_channel::Receiver<Result<(), String>>>,

    // Tournament waiting-room chat
    /// Chat message history: (display_name, message_text)
    pub chat_messages: Vec<(String, String)>,
    /// Receive channel for inbound chat messages from VPS WebSocket.
    pub chat_rx: Option<crossbeam_channel::Receiver<(String, String)>>,
    /// Current compose input.
    pub chat_input: String,
    /// Send closure for outbound chat (sends to VPS ws endpoint).
    pub chat_tx: Option<crossbeam_channel::Sender<(String, String)>>,

    /// In-flight `/my-status` poll (see `poll_my_tournament_status`).
    pub status_rx: Option<
        crossbeam_channel::Receiver<
            Result<crate::multiplayer::network::vps::MyTournamentStatus, String>,
        >,
    >,
    /// Latest backend-reported position in the active tournament. Drives the
    /// waiting-room / eliminated / champion panels.
    pub my_state: Option<crate::multiplayer::network::vps::PlayerState>,
    /// Finishing position once known (1 = champion).
    pub placing: Option<u8>,
    /// Prize owed for `placing`, in lamports.
    pub prize_lamports: Option<u64>,

    /// Tournament IDs the player dismissed from the browser (e.g. a cancelled
    /// tournament they no longer want cluttering the list). Client-side only —
    /// `available_tournaments` is fully replaced on every poll, so this can't
    /// live on the summary itself; re-appears if the backend ever reuses the ID.
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
            chat_messages: Vec::new(),
            chat_rx: None,
            chat_input: String::new(),
            chat_tx: None,
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
        *self = Self::default();
    }

    /// Start the background WebSocket chat for `tournament_id`.
    /// Polls `/api/tournament/{id}/chat` (GET for history, POST to send).
    /// Uses a simple long-poll pattern so no ws dependency is needed.
    pub fn start_chat(&mut self, tournament_id: u64, player_name: String) {
        if self.chat_rx.is_some() {
            return;
        }
        let (inbound_tx, inbound_rx) = crossbeam_channel::bounded::<(String, String)>(64);
        let (outbound_tx, outbound_rx) = crossbeam_channel::bounded::<(String, String)>(16);
        self.chat_rx = Some(inbound_rx);
        self.chat_tx = Some(outbound_tx);
        let base = crate::multiplayer::network::vps::vps_base();
        // Background thread: poll for new messages every 3 seconds
        let base2 = base.clone();
        let itx = inbound_tx.clone();
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            let mut last_id: u64 = 0;
            loop {
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
                            let _ = itx.send((sender, text));
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        });
        // Background thread: send outbound messages
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            while let Ok((player, text)) = outbound_rx.recv() {
                let url = format!("{}/api/tournament/{}/chat", base, tournament_id);
                let body = serde_json::json!({ "player": player, "text": text });
                let _ = client.post(&url).json(&body).send();
                let _ = player; // suppress move warning
            }
        });
        let _ = player_name;
    }

    /// Drain inbound chat channel into `chat_messages`.
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

/// Spawn the real on-chain `register_player` transaction (deposits the entry
/// fee into escrow and adds the player to their tournament shard) on a
/// background thread. This is separate from — and should run before —
/// `vps::confirm_join`, which only maintains the off-chain bracket/roster
/// and never touches the chain. `password` is unused here (password gating
/// lives entirely in the off-chain join call); kept for signature stability.
pub fn spawn_register_tournament(
    tournament_id: u64,
    wallet_pubkey: Pubkey,
    _password: Option<String>,
) -> oneshot::Receiver<Result<usize, String>> {
    let (tx, rx) = oneshot::channel();
    let rpc_url = std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    std::thread::spawn(move || {
        let res = register_tournament(tournament_id, wallet_pubkey, &rpc_url);
        let _ = tx.send(res.map(|_| 0usize));
    });
    rx
}

pub fn spawn_swiss_subscription(
    tournament_id: u64,
    round_tx: mpsc::UnboundedSender<SwissRoundStartedEvent>,
    result_tx: mpsc::UnboundedSender<SwissResultRecordedEvent>,
    standings_tx: mpsc::UnboundedSender<SwissStandingsUpdatedEvent>,
    bracket_fired_tx: crossbeam_channel::Sender<BracketFiredEvent>,
) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            use braid_chess::{MatchResult, SwissMessage};
            use braid_iroh::BraidIrohNode;
            use braid_iroh::DiscoveryConfig;

            let vps_url = crate::multiplayer::network::vps::vps_base();

            // Get Iroh node ID from VPS
            let client = reqwest::Client::new();
            let node_id_res = client.get(format!("{}/iroh/node-id", vps_url)).send().await;

            let _node_id = match node_id_res {
                Ok(resp) if resp.status().is_success() => match resp.text().await {
                    Ok(text) => text,
                    Err(e) => {
                        error!("Failed to read Iroh node ID response: {}", e);
                        return;
                    }
                },
                Ok(resp) => {
                    error!(
                        "Failed to get Iroh node ID from VPS: HTTP {}",
                        resp.status()
                    );
                    return;
                }
                Err(e) => {
                    error!("Failed to get Iroh node ID from VPS: {}", e);
                    return;
                }
            };

            // Create Iroh node using BraidIrohConfig
            use braid_iroh::BraidIrohConfig;
            let node = match BraidIrohNode::spawn(BraidIrohConfig {
                discovery: DiscoveryConfig::Real,
                secret_key: None,
                proxy_config: None,
                data_dir: None,
            })
            .await
            {
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
                // The topic carries Braid updates for every one of this
                // tournament's resources; `SwissMessage::from_update` reads the
                // resource path off the update and returns `None` for the ones
                // this client has no event for (roster, meta).
                let Ok(update) = serde_json::from_slice::<braid_core::Update>(&payload) else {
                    continue;
                };
                let Some(swiss_msg) = SwissMessage::from_update(&update) else {
                    continue;
                };
                match swiss_msg {
                    SwissMessage::RoundStarted {
                        round, pairings, ..
                    } => {
                        let _ = round_tx.send(SwissRoundStartedEvent {
                            tournament_id,
                            round,
                            pairings: pairings
                                .iter()
                                .map(|p| (p.white.clone(), p.black.clone()))
                                .collect(),
                        });
                    }
                    SwissMessage::ResultRecorded {
                        round,
                        board,
                        result,
                        ..
                    } => {
                        // `SwissMessage::ResultRecorded` carries only the match outcome;
                        // white/black player identifiers are resolved from the pairing
                        // stored alongside the round.
                        let (white, black) = match &result {
                            MatchResult::Win { winner } => (winner.clone(), String::new()),
                            MatchResult::Draw => (String::new(), String::new()),
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
                    SwissMessage::StandingsUpdated { standings, .. } => {
                        let _ = standings_tx.send(SwissStandingsUpdatedEvent {
                            tournament_id,
                            standings: standings
                                .iter()
                                .map(|s| (s.player_id.clone(), s.score, s.rank))
                                .collect(),
                        });
                    }
                    SwissMessage::BracketFired {
                        player_count,
                        started_at,
                        ..
                    } => {
                        let _ = bracket_fired_tx.send(BracketFiredEvent {
                            tournament_id,
                            player_count,
                            started_at,
                        });
                    }
                }
            }
        })
        .detach();
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
    menu_state: Option<Res<State<crate::core::MenuState>>>,
) {
    let is_tournaments_active = menu_state
        .map(|s| *s.get() == crate::core::MenuState::Tournaments)
        .unwrap_or(false);
    let poll_interval_secs: f64 = if is_tournaments_active { 10.0 } else { 30.0 };

    if let Some(ref rx) = tournament.list_rx {
        match rx.try_recv() {
            Ok(Ok(list)) => {
                tournament.available_tournaments = list;
                tournament.last_poll_error = None;
                tournament.list_rx = None;
            }
            Ok(Err(e)) => {
                tournament.last_poll_error = Some(e);
                tournament.list_rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                tournament.last_poll_error =
                    Some("List poll task ended without responding".to_string());
                tournament.list_rx = None;
            }
        }
    }

    if tournament.list_rx.is_some() {
        return;
    }

    let should_poll = tournament
        .last_list_poll
        .map(|t| t.elapsed().as_secs_f64() >= poll_interval_secs)
        .unwrap_or(true);

    if !should_poll {
        return;
    }

    tournament.last_list_poll = Some(Instant::now());
    let (tx, rx) = crossbeam_channel::bounded(1);
    tournament.list_rx = Some(rx);

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = crate::multiplayer::network::vps::list_tournaments();
            if let Err(ref e) = result {
                warn!("[TOURNAMENT] list_tournaments failed: {}", e);
            }
            let _ = tx.send(result);
        })
        .detach();
}

/// Polls `GET /api/tournament/{id}/games` for the tournament the viewer has
/// expanded, every 3 seconds.
///
/// One request for one tournament, against the cross-tournament walk's request
/// *per advertised tournament* — so this can afford to be three times as fresh
/// while doing a fraction of the work. Everything the Live/Upcoming/Finished
/// tabs need arrives already computed.
fn poll_expanded_tournament_games(mut tournament: ResMut<TournamentClientState>) {
    if let Some(ref rx) = tournament.detail_games_rx {
        match rx.try_recv() {
            Ok(Ok(games)) => {
                tournament.detail_games = games;
                tournament.detail_games_rx = None;
            }
            Ok(Err(e)) => {
                warn!("[TOURNAMENT] tournament games fetch failed: {e}");
                tournament.detail_games_rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => tournament.detail_games_rx = None,
        }
    }

    let Some(expanded) = tournament.expanded_tournament_id else {
        // Collapsed: drop the list so a stale tournament's games can't render
        // under a different card later.
        if tournament.detail_games_for.is_some() {
            tournament.detail_games.clear();
            tournament.detail_games_for = None;
        }
        return;
    };

    // Switching cards must not show the previous tournament's games.
    if tournament.detail_games_for != Some(expanded) {
        tournament.detail_games.clear();
        tournament.detail_games_for = Some(expanded);
        tournament.last_detail_poll = None;
    }

    if tournament.detail_games_rx.is_some() {
        return;
    }
    let should_poll = tournament
        .last_detail_poll
        .map(|t| t.elapsed().as_secs() >= 3)
        .unwrap_or(true);
    if !should_poll {
        return;
    }

    tournament.last_detail_poll = Some(Instant::now());
    let (tx, rx) = crossbeam_channel::bounded(1);
    tournament.detail_games_rx = Some(rx);

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let _ = tx.send(crate::multiplayer::network::vps::fetch_tournament_games(
                expanded,
            ));
        })
        .detach();
}

/// Polls the on-chain tournament-games list (bracket matches with a Solana
/// game_id) while the Tournaments browser is open, every 10 seconds. Feeds the
/// per-tournament "Watch Live" list.
fn poll_tournament_games_list(
    mut tournament: ResMut<TournamentClientState>,
    menu_state: Option<Res<State<crate::core::MenuState>>>,
) {
    if let Some(ref rx) = tournament.games_rx {
        match rx.try_recv() {
            Ok(games) => {
                tournament.tournament_games = games;
                tournament.games_rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                tournament.games_rx = None;
            }
        }
    }

    // Only fetch while the browser is actually open — the fetch walks one
    // bracket request per advertised tournament.
    let is_tournaments_active = menu_state
        .map(|s| *s.get() == crate::core::MenuState::Tournaments)
        .unwrap_or(false);
    if !is_tournaments_active || tournament.games_rx.is_some() {
        return;
    }
    let should_poll = tournament
        .last_games_poll
        .map(|t| t.elapsed().as_secs() >= 10)
        .unwrap_or(true);
    if !should_poll {
        return;
    }

    tournament.last_games_poll = Some(Instant::now());
    let (tx, rx) = crossbeam_channel::bounded(1);
    tournament.games_rx = Some(rx);

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            match crate::multiplayer::network::vps::list_tournament_games() {
                Ok(games) => {
                    let _ = tx.send(games);
                }
                Err(e) => warn!("[TOURNAMENT] list_tournament_games failed: {}", e),
            }
        })
        .detach();
}

fn poll_bracket_fired(mut tournament: ResMut<TournamentClientState>) {
    let Some(ref rx) = tournament.bracket_fired_rx else {
        return;
    };
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

/// How often to ask the backend where we stand in the active tournament.
const MY_STATUS_POLL_SECS: f32 = 3.0;

/// Drives the client's tournament state from `GET /my-status`.
///
/// This is the producer `TournamentMatchAssignedEvent` never had. That event
/// is consumed by `handle_tournament_match_assigned`, which does *everything*
/// needed to actually play a tournament match — session key setup, P2P
/// connect, ER delegation, transition to `InGame` — but nothing in the
/// codebase ever wrote it, so the handler could never run and the client
/// could not enter any tournament match, in any round.
///
/// It also drives the between-rounds UI: `waiting_for_next_match` was only
/// ever assigned `false`, so the (fully written) waiting-room panel in
/// `screens.rs` / `game_ui.rs` was unreachable, and a player who won round 1
/// was dropped to the menu with no sign the tournament was still running.
///
/// See docs/plans/tournament-end-to-end-fix-plan.md §5 Phase 1.
fn poll_my_tournament_status(
    time: Res<Time>,
    mut tournament: ResMut<TournamentClientState>,
    solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    mut assigned: MessageWriter<TournamentMatchAssignedEvent>,
) {
    let Some(tournament_id) = tournament.active_tournament_id else {
        return;
    };
    let Some(wallet) = solana_state.as_ref().and_then(|s| s.wallet_pubkey) else {
        return;
    };

    // Collect the in-flight response before issuing another request.
    if let Some(rx) = tournament.status_rx.as_ref() {
        match rx.try_recv() {
            Ok(Ok(status)) => {
                tournament.status_rx = None;
                apply_my_status(&mut tournament, tournament_id, status, &mut assigned);
            }
            Ok(Err(e)) => {
                tournament.status_rx = None;
                tournament.last_poll_error = Some(e);
            }
            Err(crossbeam_channel::TryRecvError::Empty) => return,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                tournament.status_rx = None;
            }
        }
    }

    tournament.poll_timer += time.delta_secs();
    if tournament.poll_timer < MY_STATUS_POLL_SECS {
        return;
    }
    tournament.poll_timer = 0.0;

    let (tx, rx) = crossbeam_channel::bounded(1);
    tournament.status_rx = Some(rx);
    let wallet_str = wallet.to_string();
    std::thread::spawn(move || {
        let result =
            crate::multiplayer::network::vps::my_tournament_status(tournament_id, &wallet_str);
        let _ = tx.send(result);
    });
}

/// Translates a `/my-status` snapshot into client state + events.
fn apply_my_status(
    tournament: &mut TournamentClientState,
    tournament_id: u64,
    status: crate::multiplayer::network::vps::MyTournamentStatus,
    assigned: &mut MessageWriter<TournamentMatchAssignedEvent>,
) {
    use crate::multiplayer::network::vps::PlayerState;

    tournament.last_poll_error = None;
    tournament.my_state = Some(status.state);
    tournament.placing = status.placing;
    tournament.prize_lamports = status.prize_lamports;

    if let Some(ref last) = status.last_result {
        tournament.last_match_result = Some(if last.won {
            format!("Round {} — win", last.round + 1)
        } else {
            format!("Round {} — loss", last.round + 1)
        });
    }

    match status.state {
        PlayerState::MatchReady => {
            let Some(m) = status.my_match else { return };
            let Some(game_id) = m.game_id else { return };
            // Already in this game — nothing to do.
            if tournament.active_game_id == Some(game_id) {
                return;
            }
            tournament.waiting_for_next_match = false;
            tournament.status_message = format!(
                "Round {} — your match is ready",
                status.round.map(|r| r + 1).unwrap_or(1)
            );
            info!(
                "[TOURNAMENT] Match ready in tournament {} (game {}, vs {})",
                tournament_id, game_id, m.opponent_pubkey
            );
            assigned.write(TournamentMatchAssignedEvent {
                tournament_id,
                match_index: m.match_index as u8,
                game_id: Some(game_id),
                opponent_pubkey: m.opponent_pubkey,
                opponent_node_id: m.opponent_node_id,
                your_color: m.your_color,
            });
        }
        PlayerState::AwaitingOpponent | PlayerState::AwaitingGameId => {
            // Won (or seated) but nothing to enter yet. Clear the stale game
            // id so the next MatchReady isn't suppressed by the guard above.
            tournament.active_game_id = None;
            tournament.waiting_for_next_match = true;
            tournament.status_message = match status.blocked_by {
                Some(ref b) if b.matches_remaining > 0 => format!(
                    "Waiting for round {} to finish — {} match{} remaining",
                    b.round + 1,
                    b.matches_remaining,
                    if b.matches_remaining == 1 { "" } else { "es" }
                ),
                _ => "Waiting for your next opponent…".to_string(),
            };
        }
        PlayerState::Eliminated | PlayerState::Champion | PlayerState::Cancelled => {
            tournament.waiting_for_next_match = false;
            tournament.active_game_id = None;
            tournament.status_message = match status.state {
                PlayerState::Champion => "You won the tournament".to_string(),
                PlayerState::Cancelled => "Tournament cancelled".to_string(),
                _ => match status.placing {
                    Some(p) => format!("Eliminated — finished #{p}"),
                    None => "Eliminated".to_string(),
                },
            };
        }
        PlayerState::Registered => {
            tournament.waiting_for_next_match = false;
            tournament.status_message = "Registered — waiting for the tournament to start".into();
        }
        PlayerState::NotRegistered => {
            tournament.waiting_for_next_match = false;
        }
    }
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
                ),
            )
            .add_systems(Update, handle_tournament_match_assigned);
    }
}

/// Item 7: When a tournament match is assigned, create/join the VPS session so
/// the game session key is ready before the P2P handshake starts.
fn handle_tournament_match_assigned(
    mut events: MessageReader<TournamentMatchAssignedEvent>,
    mut tournament: ResMut<TournamentClientState>,
    solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    network_state: Res<crate::multiplayer::types::OnlineNetworkState>,
    network_config: Res<crate::multiplayer::types::NetworkConfig>,
    mut online_session: ResMut<crate::multiplayer::network::online_game_session::OnlineGameSession>,
    mut p2p_state: ResMut<crate::multiplayer::network::p2p::P2PConnectionState>,
    mut connect_events: MessageWriter<crate::multiplayer::network::p2p::ConnectToPeerEvent>,
    mut core_mode: ResMut<crate::core::GameMode>,
    mut ai_config: ResMut<crate::game::ai::ChessAIResource>,
    mut next_game_state: ResMut<NextState<crate::core::GameState>>,
    mut game_started: MessageWriter<crate::game::events::GameStartedEvent>,
    mut solana_sync: ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
    mut competitive: ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
) {
    for ev in events.read() {
        let game_id = match ev.game_id {
            Some(id) => id,
            None => {
                warn!(
                    "[TOURNAMENT] Match assigned for tournament {} but game_id is None — skipping session setup",
                    ev.tournament_id
                );
                continue;
            }
        };

        if tournament.active_game_id == Some(game_id) {
            continue;
        }

        let wallet_str = solana_state
            .as_ref()
            .and_then(|s| s.wallet_pubkey)
            .map(|pk| pk.to_string())
            .unwrap_or_default();

        if wallet_str.is_empty() {
            warn!(
                "[TOURNAMENT] No wallet for tournament {} game {} session setup",
                ev.tournament_id, game_id
            );
            continue;
        }

        let tournament_id = ev.tournament_id;
        let is_white = ev.your_color == "white";
        let wallet = wallet_str.clone();
        let game_id_str = game_id.to_string();
        let local_node_id = network_state
            .node_id
            .as_ref()
            .map(|id| bs58::encode(id.as_bytes()).into_string());
        let opponent_node_id = ev.opponent_node_id.clone();

        if let Some(ref peer_node_id) = opponent_node_id {
            // Both tournament participants fire this event toward each other
            // symmetrically — `is_white` is the arbitrary but consistent
            // tie-breaker so exactly one side ends up `is_host` (whichever
            // side auto-accepts the other's GameInvite and sends back
            // InviteResponse). Without a consistent split here, both sides
            // would hit the 12s Connecting timeout every time.
            // Non-authoritative: this system already drives game-start itself
            // below, unconditionally — this connect is purely an opportunistic
            // dual-transport upgrade, same as the VPS-lobby flow.
            connect_events.write(crate::multiplayer::network::p2p::ConnectToPeerEvent {
                peer_node_id: peer_node_id.clone(),
                is_host: is_white,
                drive_game_start: false,
            });
        }

        if let Some(ref node_id) = local_node_id {
            announce_or_join_tournament_relay(
                game_id_str.clone(),
                node_id.clone(),
                wallet_str.clone(),
                is_white,
            );
        } else {
            warn!(
                "[TOURNAMENT] Local online node id not ready for relay setup on game {}",
                game_id
            );
        }

        crate::multiplayer::network::online_game_session::start_session(
            &mut online_session,
            network_config.vps_base_url.clone(),
            game_id_str,
            0.0,
            &network_state,
        );

        p2p_state.local_node_id = network_state.node_id;
        p2p_state.peer_node_id = opponent_node_id;
        p2p_state.game_id = Some(game_id);
        p2p_state.is_host = is_white;
        p2p_state.player_color = Some(if is_white {
            crate::rendering::pieces::PieceColor::White
        } else {
            crate::rendering::pieces::PieceColor::Black
        });
        p2p_state.status = crate::multiplayer::network::p2p::P2PConnectionStatus::InGame;

        *core_mode = crate::core::GameMode::OnlineMultiplayer;
        ai_config.mode = crate::game::ai::resource::GameMode::Multiplayer;
        solana_sync.game_id = Some(game_id);
        solana_sync.wager_amount = 0;
        // Tournament games run through ER delegation even with a zero wager.
        solana_sync.requires_delegation = true;
        competitive.game_id = Some(game_id);
        competitive.wager_lamports = 0;
        competitive.active = true;
        rollup_manager.game_id = game_id;
        rollup_manager.is_creator = is_white;
        crate::multiplayer::network::game_id_store::set(game_id);

        game_started.write(crate::game::events::GameStartedEvent { game_id });
        next_game_state.set(crate::core::GameState::InGame);
        tournament.active_game_id = Some(game_id);
        tournament.waiting_for_next_match = false;

        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                use crate::multiplayer::vps_client;
                if is_white {
                    match vps_client::tournament_session_create_game(
                        tournament_id,
                        game_id,
                        &wallet,
                    ) {
                        Ok(session_pk) => info!(
                            "[TOURNAMENT] Session created for game {} → {}",
                            game_id, session_pk
                        ),
                        Err(e) => error!(
                            "[TOURNAMENT] session-create-game failed for game {}: {e}",
                            game_id
                        ),
                    }
                } else {
                    match vps_client::tournament_session_join_game(tournament_id, game_id, &wallet)
                    {
                        Ok(session_pk) => info!(
                            "[TOURNAMENT] Session joined for game {} → {}",
                            game_id, session_pk
                        ),
                        Err(e) => error!(
                            "[TOURNAMENT] session-join-game failed for game {}: {e}",
                            game_id
                        ),
                    }
                }
            })
            .detach();

        tournament.status_message = format!(
            "Match assigned — setting up game session for game {}…",
            game_id
        );
    }
}

fn announce_or_join_tournament_relay(
    game_id: String,
    local_node_id: String,
    wallet: String,
    is_white: bool,
) {
    std::thread::spawn(move || {
        use crate::multiplayer::vps_client;
        if is_white {
            match vps_client::p2p_announce_game(
                game_id.clone(),
                &local_node_id,
                &format!("Tournament {}", &wallet[..wallet.len().min(8)]),
                0.0,
                "tournament_match",
                0,
                0,
                Some(wallet),
                None,
                None,
            ) {
                Ok(()) => info!("[TOURNAMENT] Relay announced game {}", game_id),
                Err(e) => warn!(
                    "[TOURNAMENT] Relay announce failed for game {}: {}",
                    game_id, e
                ),
            }
        } else {
            for attempt in 1..=15 {
                match vps_client::p2p_join_game(game_id.clone(), &local_node_id) {
                    Ok(Some(host_id)) => {
                        info!(
                            "[TOURNAMENT] Relay joined game {} via host {}",
                            game_id, host_id
                        );
                        return;
                    }
                    Ok(None) => {
                        info!("[TOURNAMENT] Relay joined game {}", game_id);
                        return;
                    }
                    Err(e) => {
                        if attempt == 15 {
                            warn!(
                                "[TOURNAMENT] Relay join failed for game {} after retries: {}",
                                game_id, e
                            );
                        } else {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                        }
                    }
                }
            }
        }
    });
}

/// Registration info needed to build `register_player_ix`, fetched from the
/// backend rather than derived locally — `host_treasury` isn't something the
/// client can compute (it's the VPS operator's own key, stamped onto the
/// on-chain Tournament account at creation).
#[derive(Deserialize)]
struct RegistrationInfo {
    program_id: String,
    max_players: u16,
    host_treasury: String,
}

fn fetch_registration_info(rpc_base: &str, tournament_id: u64) -> Result<RegistrationInfo, String> {
    let resp = crate::multiplayer::network::vps::client_fast()?
        .get(format!(
            "{}/api/tournament/{}/registration-info",
            rpc_base, tournament_id
        ))
        .send()
        .map_err(|e| format!("fetch registration-info: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("fetch registration-info: HTTP {}", resp.status()));
    }
    resp.json::<RegistrationInfo>()
        .map_err(|e| format!("parse registration-info: {e}"))
}

/// Submits the real on-chain `register_player` transaction: fetches the data
/// needed to build it (program ID, tournament size, host treasury) from the
/// backend, builds the instruction client-side, signs it via the Tauri
/// wallet bridge (the same "one popup" mechanism used for wagered games —
/// see `lobby::async_create_game`), and confirms on-chain before returning.
/// Returns the confirmed `register_player` transaction signature. The caller
/// must hand that signature to `vps::confirm_join`, which is what actually
/// puts the player on the backend's roster: the backend re-reads the tx from
/// chain and verifies the player signed a real `register_player` for this
/// tournament before trusting it. Previously this returned a bare `Ok(0)` and
/// dropped the signature on the floor, which left the caller with nothing to
/// confirm with.
pub fn register_tournament(
    tournament_id: u64,
    wallet_pubkey: Pubkey,
    rpc_url: &str,
) -> Result<String, String> {
    use crate::multiplayer::network::vps::vps_base;
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;
    use crate::multiplayer::solana::tournament_session::build_register_player_ix;
    use std::str::FromStr;

    let info = fetch_registration_info(&vps_base(), tournament_id)?;
    let program_id =
        Pubkey::from_str(&info.program_id).map_err(|e| format!("bad program_id: {e}"))?;
    let host_treasury =
        Pubkey::from_str(&info.host_treasury).map_err(|e| format!("bad host_treasury: {e}"))?;

    // Matches the ELO the off-chain roster join call already uses (see
    // `vps::confirm_join`) — real ELO-based seeding is a future
    // enhancement, not something this registration tx needs to solve.
    const DEFAULT_ELO: u32 = 1200;

    let ix = build_register_player_ix(
        &program_id,
        tournament_id,
        info.max_players,
        &wallet_pubkey,
        &host_treasury,
        DEFAULT_ELO,
    );

    let sig = sign_and_send_via_tauri(
        rpc_url,
        wallet_pubkey,
        &[ix],
        &[],
        "Registering for tournament",
    )
    .map_err(|e| format!("register_player tx: {e}"))?;

    info!(
        "[TOURNAMENT] register_player confirmed for tournament {} ({})",
        tournament_id, sig
    );
    Ok(sig.to_string())
}
