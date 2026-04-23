//! P2P VPS Relay Network Module
//!
//! Handles P2P game hosting and joining via VPS relay instead of direct Iroh gossip.
//! This provides reliable NAT traversal and eliminates manual Node ID sharing.

#![allow(dead_code)]

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::multiplayer::{
    vps_client,
    BraidNetworkState,
};
use crate::multiplayer::network::p2p::{
    ConnectToPeerEvent, P2PConnectionState,
};

use crossbeam_channel::{Receiver, Sender};

/// Resource for P2P VPS relay state
#[derive(Resource)]
pub struct P2PVpsState {
    /// Whether to use VPS relay (true) or direct Iroh (false)
    pub use_vps_relay: bool,
    /// Last time we polled for games
    pub last_poll: Option<std::time::Instant>,
    /// Cached game listings
    pub cached_games: Vec<VpsGameListing>,
    /// Message queue for outgoing messages
    pub outgoing_queue: VecDeque<(String, String)>, // (game_id, message_json)
    /// Poll index for messages
    pub poll_index: usize,
    /// Channel for background VPS responses
    pub response_tx: Sender<VpsResponse>,
    pub response_rx: Receiver<VpsResponse>,
}

impl Default for P2PVpsState {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            use_vps_relay: false,
            last_poll: None,
            cached_games: Vec::new(),
            outgoing_queue: VecDeque::new(),
            poll_index: 0,
            response_tx: tx,
            response_rx: rx,
        }
    }
}

/// Result of a background VPS operation
pub enum VpsResponse {
    GameList(Vec<vps_client::P2PGameListing>),
    JoinResult {
        game_id: String,
        host_node_id: Option<String>,
        stake_amount: f64,
    },
    Error(String),
}

/// Game listing from VPS
#[derive(Debug, Clone)]
pub struct VpsGameListing {
    pub game_id: String,
    pub display_name: String,
    pub stake_amount: f64,
    pub game_type: String,
    pub time_control_minutes: u32,
    pub username: Option<String>,
    pub elo: Option<u16>,
    pub region: Option<String>,
}

/// Plugin for P2P VPS relay
pub struct P2PVpsPlugin;

impl Plugin for P2PVpsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<P2PVpsState>()
            .add_systems(Update, sync_vps_relay_settings)
            .add_systems(Update, poll_vps_game_list)
            .add_systems(Update, handle_vps_responses)
            .add_systems(Update, send_vps_messages);
    }
}

/// Sync VPS relay mode with global game settings
fn sync_vps_relay_settings(
    settings: Res<crate::core::resources::GameSettings>,
    mut vps_state: ResMut<P2PVpsState>,
) {
    if settings.use_vps_relay != vps_state.use_vps_relay {
        vps_state.use_vps_relay = settings.use_vps_relay;
        info!("[P2P VPS] Relay mode updated from settings: {}", 
            if vps_state.use_vps_relay { "enabled" } else { "disabled" });
    }
}

/// Poll VPS for available games using IoTaskPool to avoid blocking
fn poll_vps_game_list(
    mut vps_state: ResMut<P2PVpsState>,
) {
    if !vps_state.use_vps_relay {
        return;
    }

    // Poll every 5 seconds
    let should_poll = vps_state
        .last_poll
        .map(|t| t.elapsed().as_secs() > 5)
        .unwrap_or(true);

    if !should_poll {
        return;
    }

    // Mark as polled to avoid spawning multiple tasks
    vps_state.last_poll = Some(std::time::Instant::now());

    let tx = vps_state.response_tx.clone();

    // Spawn async task for network I/O
    bevy::tasks::IoTaskPool::get().spawn(async move {
        match vps_client::p2p_list_games() {
            Ok(games) => {
                let _ = tx.send(VpsResponse::GameList(games));
            }
            Err(e) => {
                let _ = tx.send(VpsResponse::Error(format!("Failed to poll games: {}", e)));
            }
        }
    }).detach();
}

/// Handle background VPS responses and update state
fn handle_vps_responses(
    mut vps_state: ResMut<P2PVpsState>,
    mut connect_events: MessageWriter<ConnectToPeerEvent>,
    mut core_mode: ResMut<crate::core::GameMode>,
    mut ai_config: ResMut<crate::game::ai::ChessAIResource>,
    mut menu_state: ResMut<NextState<crate::core::MenuState>>,
    #[cfg(feature = "solana")]
    mut solana_lobby: Option<ResMut<crate::multiplayer::solana::lobby::SolanaLobbyState>>,
) {
    while let Ok(response) = vps_state.response_rx.try_recv() {
        match response {
            VpsResponse::GameList(games) => {
                vps_state.cached_games = games
                    .into_iter()
                    .map(|g| VpsGameListing {
                        game_id: g.game_id,
                        display_name: g.display_name,
                        stake_amount: g.stake_amount,
                        game_type: g.game_type,
                        time_control_minutes: g.time_control_minutes,
                        username: g.username,
                        elo: g.elo,
                        region: g.region,
                    })
                    .collect();
                trace!("[P2P VPS] Updated cached games: {} found", vps_state.cached_games.len());
            }
            VpsResponse::JoinResult { game_id, host_node_id, stake_amount } => {
                if let Some(host_id) = host_node_id {
                    info!("[P2P VPS] Successfully joined game {} (Stake: {})! Host: {}", game_id, stake_amount, host_id);
                    
                    // Trigger connection
                    connect_events.write(ConnectToPeerEvent {
                        peer_node_id: host_id,
                    });
                    
                    if stake_amount > 0.0 {
                        #[cfg(feature = "solana")]
                        {
                            info!("[P2P VPS] Wager detected. Transitioning to Solana Lobby for contract signing...");
                            menu_state.set(crate::core::MenuState::SolanaLobby);
                            
                            if let Some(ref mut lobby) = solana_lobby {
                                lobby.game_id_input = game_id;
                                lobby.wager_sol = stake_amount as f32;
                                lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Join;
                            }
                        }
                    } else {
                        ai_config.mode = crate::game::ai::resource::GameMode::Multiplayer;
                        *core_mode = crate::core::GameMode::BraidMultiplayer;
                    }
                } else {
                    warn!("[P2P VPS] Join for game {} rejected by host", game_id);
                }
            }
            VpsResponse::Error(e) => {
                error!("[P2P VPS] Background error: {}", e);
            }
        }
    }
}

/// Send queued messages via VPS
fn send_vps_messages(
    mut vps_state: ResMut<P2PVpsState>,
    _p2p_state: Res<P2PConnectionState>,
    network_state: Res<BraidNetworkState>,
) {
    if !vps_state.use_vps_relay {
        return;
    }

    let node_id = match network_state.node_id.as_ref() {
        Some(id) => bs58::encode(id.as_bytes()).into_string(),
        None => return,
    };

    // Process outgoing queue
    while let Some((game_id, message)) = vps_state.outgoing_queue.pop_front() {
        let game_id_clone = game_id.clone();
        if let Err(e) = vps_client::p2p_send_message(game_id_clone, &node_id, &message) {
            error!("[P2P VPS] Failed to send message: {}", e);
            // Re-queue for retry
            vps_state.outgoing_queue.push_back((game_id, message));
            break; // Stop processing to avoid spamming failed requests
        }
    }
}

/// Queue a message to be sent via VPS
pub fn queue_vps_message(vps_state: &mut P2PVpsState, game_id: String, message: String) {
    if vps_state.use_vps_relay {
        vps_state.outgoing_queue.push_back((game_id, message));
    }
}

/// Toggle VPS relay mode
pub fn set_vps_relay_mode(vps_state: &mut P2PVpsState, enabled: bool) {
    vps_state.use_vps_relay = enabled;
    info!("[P2P VPS] Relay mode: {}", if enabled { "enabled" } else { "disabled" });
}
