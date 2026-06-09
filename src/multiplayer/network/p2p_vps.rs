//! P2P VPS Relay Network Module
//!
//! Handles P2P game hosting and joining via VPS relay instead of direct Iroh gossip.
//! This provides reliable NAT traversal and eliminates manual Node ID sharing.
//!
//! Connection flow (HTTP relay only — no Iroh required):
//!   Host: announces → waits → polls relay for JOIN_ACK → starts game
//!   Joiner: sees listing → clicks join → sends JOIN_ACK → starts game immediately

#![allow(dead_code)]

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::multiplayer::{
    vps_client,
    BraidNetworkState,
};
use crate::multiplayer::network::p2p::{
    ConnectToPeerEvent, P2PConnectionState, P2PConnectionStatus,
};
use crate::game::events::GameStartedEvent;
use crate::core::states::GameState;

use crossbeam_channel::{Receiver, Sender};

/// Resource for P2P VPS relay state
#[derive(Resource)]
pub struct P2PVpsState {
    /// Whether to use VPS relay (true) or direct Iroh (false)
    pub use_vps_relay: bool,
    /// Last time we polled the game listing
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

    // ── Host-side join detection ─────────────────────────────────────────────
    /// The game_id we are currently hosting (set when hosting starts, cleared on cancel or join)
    pub hosting_game_id: Option<String>,
    /// Our own node ID (base58) — used as the "from" ID when polling relay messages
    pub hosting_node_id: Option<String>,
    /// Last time we polled the relay for joiner messages
    pub host_poll_last: Option<std::time::Instant>,
    /// Stake amount (SOL) set when announcing a wagered game. Carried into
    /// the JoinerDetected handler so the host can be routed to the Solana
    /// contract creation flow before entering InGame.
    pub hosting_stake_amount: f64,
}

impl Default for P2PVpsState {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            use_vps_relay: true,
            last_poll: None,
            cached_games: Vec::new(),
            outgoing_queue: VecDeque::new(),
            poll_index: 0,
            response_tx: tx,
            response_rx: rx,
            hosting_game_id: None,
            hosting_node_id: None,
            host_poll_last: None,
            hosting_stake_amount: 0.0,
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
    /// Host received a JOIN_ACK from the joiner via backend relay
    JoinerDetected {
        game_id: String,
        joiner_node_id: String,
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
    pub base_time_seconds: u32,
    pub increment_seconds: u16,
    pub username: Option<String>,
    pub elo: Option<u16>,
    pub region: Option<String>,
    pub capacity: u8,
    pub players_joined: u8,
    pub ttl_seconds: i64,
    pub is_private: bool,
}

/// Plugin for P2P VPS relay
pub struct P2PVpsPlugin;

impl Plugin for P2PVpsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<P2PVpsState>()
            .add_systems(Update, sync_vps_relay_settings)
            .add_systems(Update, poll_vps_game_list)
            .add_systems(Update, poll_for_joiner_messages)
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

/// Poll VPS for available game listings every 5 seconds.
fn poll_vps_game_list(mut vps_state: ResMut<P2PVpsState>) {
    if !vps_state.use_vps_relay {
        return;
    }

    let should_poll = vps_state
        .last_poll
        .map(|t| t.elapsed().as_secs() > 5)
        .unwrap_or(true);

    if !should_poll {
        return;
    }

    vps_state.last_poll = Some(std::time::Instant::now());

    let tx = vps_state.response_tx.clone();
    std::thread::spawn(move || {
        match vps_client::p2p_list_games() {
            Ok(games) => { let _ = tx.send(VpsResponse::GameList(games)); }
            Err(e)    => { let _ = tx.send(VpsResponse::Error(e)); }
        }
    });
}

/// Host polls backend relay every 2 seconds looking for a JOIN_ACK from a joiner.
/// When found, fires `VpsResponse::JoinerDetected` which starts the game on the host side.
fn poll_for_joiner_messages(mut vps_state: ResMut<P2PVpsState>) {
    let game_id = match vps_state.hosting_game_id.clone() {
        Some(id) => id,
        None => return,
    };
    let node_id = match vps_state.hosting_node_id.clone() {
        Some(id) => id,
        None => return,
    };

    let should_poll = vps_state.host_poll_last
        .map(|t| t.elapsed().as_secs() >= 2)
        .unwrap_or(true);

    if !should_poll {
        return;
    }
    vps_state.host_poll_last = Some(std::time::Instant::now());

    let tx = vps_state.response_tx.clone();
    std::thread::spawn(move || {
        match vps_client::p2p_poll_messages(game_id.clone(), &node_id, 0) {
            Ok((messages, _)) => {
                for msg in &messages {
                    if let Some(joiner_id) = msg.strip_prefix("JOIN_ACK:") {
                        info!("[P2P VPS] JOIN_ACK received from {}", joiner_id);
                        let _ = tx.send(VpsResponse::JoinerDetected {
                            game_id,
                            joiner_node_id: joiner_id.to_string(),
                        });
                        return;
                    }
                }
            }
            Err(e) => tracing::debug!("[P2P VPS] Host relay poll: {}", e),
        }
    });
}

/// Handle background VPS responses and update state
#[allow(clippy::too_many_arguments)]
fn handle_vps_responses(
    mut vps_state: ResMut<P2PVpsState>,
    mut connect_events: MessageWriter<ConnectToPeerEvent>,
    mut core_mode: ResMut<crate::core::GameMode>,
    mut ai_config: ResMut<crate::game::ai::ChessAIResource>,
    #[allow(unused_mut, unused_variables)] mut menu_state: ResMut<NextState<crate::core::MenuState>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut game_started: MessageWriter<GameStartedEvent>,
    mut p2p_conn: ResMut<P2PConnectionState>,
    #[cfg(feature = "solana")]
    mut solana_lobby: Option<ResMut<crate::multiplayer::solana::lobby::SolanaLobbyState>>,
    mut braid_pvp_session: ResMut<crate::multiplayer::network::braid_pvp::BraidPvpSession>,
    network_config: Res<crate::multiplayer::types::NetworkConfig>,
    network_state: Res<crate::multiplayer::BraidNetworkState>,
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
                        base_time_seconds: g.base_time_seconds,
                        increment_seconds: g.increment_seconds,
                        username: g.username,
                        elo: g.elo,
                        region: g.region,
                        capacity: g.capacity,
                        players_joined: g.players_joined,
                        ttl_seconds: g.ttl_seconds,
                        is_private: g.is_private,
                    })
                    .collect();
                trace!("[P2P VPS] Updated cached games: {} found", vps_state.cached_games.len());
            }

            VpsResponse::JoinResult { game_id, host_node_id, stake_amount } => {
                if let Some(host_id) = host_node_id {
                    info!("[P2P VPS] Joined game {} (stake={}). Host node: {}", game_id, stake_amount, host_id);

                    // Opportunistically try Iroh P2P (dual transport — OK if it fails)
                    connect_events.write(ConnectToPeerEvent {
                        peer_node_id: host_id.clone(),
                    });

                    // Start Braid-HTTP relay session for reliable move transport
                    crate::multiplayer::network::braid_pvp::start_session(
                        &mut braid_pvp_session,
                        network_config.vps_base_url.clone(),
                        game_id.clone(),
                        stake_amount,
                        &network_state,
                    );

                    // Ask the host for the authoritative board state.
                    // On a fresh game this is a no-op (host replies with starting FEN).
                    // On reconnect this restores the current mid-game position.
                    let gid = parse_game_id_u64(&game_id);
                    if let Some(tx) = &network_state.message_sender {
                        let _ = tx.send(crate::multiplayer::network::protocol::NetworkMessage::ResyncRequest { game_id: gid });
                        info!("[P2P VPS] Sent ResyncRequest to host for game {gid}");
                    }

                    if stake_amount > 0.0 {
                        #[cfg(feature = "solana")]
                        {
                            info!("[P2P VPS] Wager — transitioning to Solana Lobby for signing...");
                            menu_state.set(crate::core::MenuState::SolanaLobby);
                            if let Some(ref mut lobby) = solana_lobby {
                                lobby.game_id_input = game_id;
                                lobby.wager_sol = stake_amount as f32;
                                lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Join;
                            }
                        }
                    } else {
                        // Start game immediately on joiner side (Black)
                        ai_config.mode = crate::game::ai::resource::GameMode::Multiplayer;
                        *core_mode = crate::core::GameMode::BraidMultiplayer;
                        p2p_conn.is_host = false;
                        p2p_conn.player_color = Some(crate::rendering::pieces::PieceColor::Black);
                        p2p_conn.status = P2PConnectionStatus::InGame;

                        let gid = parse_game_id_u64(&game_id);
                        game_started.write(GameStartedEvent { game_id: gid });
                        next_game_state.set(GameState::InGame);
                        info!("[P2P VPS] Game started (joiner/Black) via HTTP relay");
                    }
                } else {
                    warn!("[P2P VPS] Join for game {} rejected by host", game_id);
                }
            }

            VpsResponse::JoinerDetected { game_id, joiner_node_id } => {
                let stake = vps_state.hosting_stake_amount;
                info!("[P2P VPS] Joiner {} connected to {} (stake={:.3} SOL). Starting as host (White).", joiner_node_id, game_id, stake);

                // Stop polling for joiners and clear stake so it isn't reused.
                vps_state.hosting_game_id = None;
                vps_state.hosting_node_id = None;
                vps_state.hosting_stake_amount = 0.0;

                // Start Braid-HTTP relay session with the correct stake amount.
                crate::multiplayer::network::braid_pvp::start_session(
                    &mut braid_pvp_session,
                    network_config.vps_base_url.clone(),
                    game_id.clone(),
                    stake,
                    &network_state,
                );

                // Opportunistically try Iroh connection to joiner
                connect_events.write(ConnectToPeerEvent {
                    peer_node_id: joiner_node_id,
                });

                // Mark connection state as host/White now so the game knows sides.
                p2p_conn.is_host = true;
                p2p_conn.player_color = Some(crate::rendering::pieces::PieceColor::White);

                if stake > 0.0 {
                    // Wagered: route host through Solana contract creation before InGame.
                    #[cfg(feature = "solana")]
                    {
                        info!("[P2P VPS] Wagered game — routing host to Solana Lobby to create on-chain game.");
                        menu_state.set(crate::core::MenuState::SolanaLobby);
                        if let Some(ref mut lobby) = solana_lobby {
                            lobby.game_id_input = game_id.clone();
                            lobby.wager_sol = stake as f32;
                            lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
                        }
                    }
                    #[cfg(not(feature = "solana"))]
                    {
                        warn!("[P2P VPS] Wagered game requested but solana feature not enabled — starting as free game.");
                        ai_config.mode = crate::game::ai::resource::GameMode::Multiplayer;
                        *core_mode = crate::core::GameMode::BraidMultiplayer;
                        p2p_conn.status = P2PConnectionStatus::InGame;
                        let gid = parse_game_id_u64(&game_id);
                        game_started.write(GameStartedEvent { game_id: gid });
                        next_game_state.set(GameState::InGame);
                    }
                } else {
                    // Free game — start immediately.
                    ai_config.mode = crate::game::ai::resource::GameMode::Multiplayer;
                    *core_mode = crate::core::GameMode::BraidMultiplayer;
                    p2p_conn.status = P2PConnectionStatus::InGame;
                    let gid = parse_game_id_u64(&game_id);
                    game_started.write(GameStartedEvent { game_id: gid });
                    next_game_state.set(GameState::InGame);
                    info!("[P2P VPS] Free game started (host/White) via HTTP relay");
                }
            }

            VpsResponse::Error(e) => {
                tracing::debug!("[P2P VPS] Background error: {}", e);
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

    while let Some((game_id, message)) = vps_state.outgoing_queue.pop_front() {
        let game_id_clone = game_id.clone();
        if let Err(e) = vps_client::p2p_send_message(game_id_clone, &node_id, &message) {
            error!("[P2P VPS] Failed to send message: {}", e);
            vps_state.outgoing_queue.push_back((game_id, message));
            break;
        }
    }
}

/// Parse the numeric suffix of a game_id string (e.g. "p2p_1947654842" → 1947654842).
fn parse_game_id_u64(game_id: &str) -> u64 {
    game_id
        .rsplit('_')
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
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
