//! Tournament client state, Bevy resources, and on-chain instruction dispatch
//! for the 4-player bracket system.

use bevy::prelude::*;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;
use crate::multiplayer::{Message, MessageWriter};

use crate::multiplayer::vps_client::TournamentSummary;

// ── Join status ────────────────────────────────────────────────────────────────

/// Tracks the state of an in-flight or completed tournament registration.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum TournamentJoinStatus {
    #[default]
    Idle,
    /// On-chain + VPS registration transaction in flight.
    Pending,
    /// Registration succeeded — stores the assigned slot (0–3).
    Registered(usize),
    Error(String),
}

// ── Resources ─────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct TournamentClientState {
    /// Currently browsed / joined tournament ID.
    pub active_tournament_id: Option<u64>,
    /// Player slot within the tournament (0–3).
    pub my_slot: Option<usize>,
    /// Cached list of available tournaments from VPS.
    pub available_tournaments: Vec<TournamentSummary>,
    /// Status text shown in lobby / waiting screens.
    pub status_message: String,
    /// Polling timer: seconds since last my-match poll.
    pub poll_timer: f32,
    /// Registration state machine.
    pub join_status: TournamentJoinStatus,
    /// Oneshot receiver for an in-flight `register_player` transaction.
    pub tx_rx: Option<oneshot::Receiver<Result<usize, String>>>,
}

impl Default for TournamentClientState {
    fn default() -> Self {
        Self {
            active_tournament_id: None,
            my_slot: None,
            available_tournaments: Vec::new(),
            status_message: String::new(),
            poll_timer: 0.0,
            join_status: TournamentJoinStatus::default(),
            tx_rx: None,
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

// ── Events ────────────────────────────────────────────────────────────────────

#[derive(Message, Debug, Clone)]
pub struct OpenTournamentDiscoveryEvent;

#[derive(Message, Debug, Clone)]
pub struct RegisterForTournamentEvent {
    pub tournament_id: u64,
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
    pub pairings: Vec<(String, String)>, // (white, black) pairs
}

#[derive(Message, Debug, Clone)]
pub struct SwissResultRecordedEvent {
    pub tournament_id: u64,
    pub round: u8,
    pub board: u16,
    pub white: String,
    pub black: String,
    pub result: String, // "win", "loss", "draw"
}

#[derive(Message, Debug, Clone)]
pub struct SwissStandingsUpdatedEvent {
    pub tournament_id: u64,
    pub standings: Vec<(String, f64, u16)>, // (player, score, rank)
}

// ── On-chain instruction dispatch ─────────────────────────────────────────────

/// Spawn a background task on `IoTaskPool` that:
///   1. Builds the `register_player` on-chain instruction.
///   2. Sends it via the Tauri signing bridge → Phantom popup → confirmed on devnet.
///   3. Notifies the VPS backend via `POST /tournament/{id}/join`.
///   4. Returns the assigned slot index (or an error) through `tx`.
pub fn spawn_register_tournament(
    tournament_id: u64,
    wallet_pubkey: Pubkey,
    elo: u32,
    tx: oneshot::Sender<Result<usize, String>>,
) {
    let program_id: Pubkey = crate::solana::instructions::PROGRAM_ID
        .parse()
        .unwrap_or_default();
    let rpc_url = crate::multiplayer::solana::integration::state::DEVNET_RPC_URL.to_string();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result =
                async_register_tournament(rpc_url, program_id, tournament_id, wallet_pubkey, elo)
                    .await;
            let _ = tx.send(result);
        })
        .detach();
}

async fn async_register_tournament(
    rpc_url: String,
    program_id: Pubkey,
    tournament_id: u64,
    wallet_pubkey: Pubkey,
    _elo: u32,
) -> Result<usize, String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;
    use crate::solana::instructions::{init_profile_ix, register_player_ix, PROFILE_SEED};
    use solana_client::rpc_client::RpcClient;

    // Check whether a PlayerProfile PDA already exists for this wallet.
    // If not, prepend init_profile so it gets created in the same transaction.
    let rpc = RpcClient::new(rpc_url.clone());
    let profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, wallet_pubkey.as_ref()],
        &program_id,
    ).0;
    let needs_profile = rpc.get_account(&profile_pda).is_err();

    let register_ix = register_player_ix(program_id, wallet_pubkey, tournament_id)
        .map_err(|e| format!("build register_player_ix: {e}"))?;

    let mut ixs = Vec::new();
    if needs_profile {
        if let Ok(ix) = init_profile_ix(program_id, wallet_pubkey) {
            ixs.push(ix);
        }
    }
    ixs.push(register_ix);

    sign_and_send_via_tauri(&rpc_url, wallet_pubkey, &ixs, &[])
        .map_err(|e| format!("wallet sign: {e}"))?;

    // Notify VPS backend of registration
    let vps_url = std::env::var("VPS_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/tournament/{}/join", vps_url, tournament_id))
        .json(&serde_json::json!({
            "player_pubkey": wallet_pubkey.to_string(),
        }))
        .send()
        .await;

    if let Err(e) = res {
        return Err(format!("VPS join failed: {}", e));
    }

    // Get assigned slot from VPS response
    let res = res.unwrap();
    let slot: usize = res.json().await.map_err(|e| format!("VPS response: {}", e))?;
    Ok(slot)
}

/// Spawn a background task to subscribe to Swiss tournament updates via Iroh gossip
pub fn spawn_swiss_subscription(
    tournament_id: u64,
    mut event_sender: MessageWriter<SwissRoundStartedEvent>,
    mut result_sender: MessageWriter<SwissResultRecordedEvent>,
    mut standings_sender: MessageWriter<SwissStandingsUpdatedEvent>,
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

        let node_id = match node_id_res {
            Ok(res) => {
                let text = res.text().await.unwrap_or_default();
                text.trim().to_string()
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
        while let Ok(msg) = rx.recv().await {
            if let Ok(text) = std::str::from_utf8(&msg) {
                if let Ok(swiss_msg) = serde_json::from_str::<braid_iroh::protocol::SwissMessage>(text) {
                    match swiss_msg {
                        braid_iroh::protocol::SwissMessage::RoundStarted { round, pairings, .. } => {
                            event_sender.send(SwissRoundStartedEvent {
                                tournament_id,
                                round,
                                pairings: pairings.iter().map(|(w, b)| (w.clone(), b.clone())).collect(),
                            });
                        }
                        braid_iroh::protocol::SwissMessage::ResultRecorded { round, board, white, black, result, .. } => {
                            result_sender.send(SwissResultRecordedEvent {
                                tournament_id,
                                round,
                                board,
                                white,
                                black,
                                result: result.to_string(),
                            });
                        }
                        braid_iroh::protocol::SwissMessage::StandingsUpdated { standings, .. } => {
                            standings_sender.write(SwissStandingsUpdatedEvent {
                                tournament_id,
                                standings: standings.iter().map(|s| (s.player_id.clone(), s.score, s.rank)).collect(),
                            });
                        }
                    }
                }
            }
        }
    }).detach();
}

// ── Bevy system ───────────────────────────────────────────────────────────────

/// Runs every frame in `Update`. Checks if the in-flight registration oneshot
/// has resolved and updates `TournamentClientState` + navigates to the lobby.
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

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct TournamentClientPlugin;

impl Plugin for TournamentClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TournamentClientState>()
            .add_systems(Update, poll_tournament_tasks);
    }
}
