use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;
use iroh::EndpointId;

use crate::core::states::GameState;
use crate::game::events::GameStartedEvent;
use crate::multiplayer::{network::protocol::NetworkMessage, NetworkEvent, OnlineNetworkState};

#[derive(Resource, Debug, Clone, Default)]
pub struct P2PConnectionState {
    pub local_node_id: Option<EndpointId>,
    pub peer_node_id: Option<String>,
    pub status: P2PConnectionStatus,
    pub game_id: Option<u64>,
    pub is_host: bool,
    pub player_color: Option<PieceColor>,
    pub connecting_since: Option<std::time::Instant>,
    pub last_rtt_ms: Option<u32>,
    pub opponent_display_name: Option<String>,
    pub drive_game_start: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum P2PConnectionStatus {
    #[default]
    Disconnected,
    Hosting,       // Waiting for peer to join
    Connecting,    // Sending invite to peer
    Connected,     // Peer accepted, waiting for game start
    InGame,        // Actively playing
    Error(String), // Connection error
}

#[derive(Message)]
pub struct ConnectToPeerEvent {
    pub peer_node_id: String,
    pub is_host: bool,
    pub drive_game_start: bool,
}

#[derive(Message)]
pub struct HostGameEvent {
    pub drive_game_start: bool,
}

#[derive(Message)]
pub struct AcceptInviteEvent {
    pub game_id: u64,
}

#[derive(Message)]
pub struct RejectInviteEvent {
    pub game_id: u64,
}

#[derive(Resource)]
pub struct P2PUIState {
    pub peer_input: String,
    pub lobby_name: String,
    pub error_message: Option<String>,
}

impl Default for P2PUIState {
    fn default() -> Self {
        Self {
            peer_input: String::new(),
            lobby_name: "Guest Player".to_string(),
            error_message: None,
        }
    }
}

impl P2PUIState {
    pub fn validate_node_id(&self) -> Result<(), String> {
        let input = self.peer_input.trim();

        if input.is_empty() {
            return Err("Please enter a Node ID".to_string());
        }

        // Iroh node IDs are 32 bytes, which encode to ~44 chars in base58
        // Allow some flexibility: minimum 40 chars, maximum 60 chars
        if input.len() < 40 {
            return Err(format!(
                "Node ID too short ({} chars). Expected ~44 characters.",
                input.len()
            ));
        }

        if input.len() > 60 {
            return Err(format!(
                "Node ID too long ({} chars). Expected ~44 characters.",
                input.len()
            ));
        }

        // Check if it's valid base58
        match bs58::decode(input).into_vec() {
            Ok(decoded) => {
                if decoded.len() != 32 {
                    return Err(format!(
                        "Invalid Node ID format: decoded to {} bytes, expected 32 bytes",
                        decoded.len()
                    ));
                }
                Ok(())
            }
            Err(e) => Err(format!(
                "Invalid Node ID format: not valid base58 encoding ({:?})",
                e
            )),
        }
    }

    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error_message = Some(message.into());
    }
}

pub struct P2PConnectionPlugin;

impl Plugin for P2PConnectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<P2PConnectionState>()
            .init_resource::<P2PUIState>()
            .add_message::<ConnectToPeerEvent>()
            .add_message::<HostGameEvent>()
            .add_message::<AcceptInviteEvent>()
            .add_message::<RejectInviteEvent>()
            .add_systems(
                Update,
                (
                    handle_host_game,
                    handle_connect_to_peer,
                    handle_network_events,
                    handle_accept_invite,
                    handle_reject_invite,
                    tick_connection_timeout,
                ),
            );
    }
}

fn handle_host_game(
    mut events: MessageReader<HostGameEvent>,
    mut connection_state: ResMut<P2PConnectionState>,
    network_state: Res<OnlineNetworkState>,
) {
    for event in events.read() {
        // Generate a random game ID
        let game_id: u64 = rand::random();

        connection_state.local_node_id = network_state.node_id.clone();
        connection_state.game_id = Some(game_id);
        connection_state.is_host = true;
        connection_state.status = P2PConnectionStatus::Hosting;
        connection_state.player_color = Some(PieceColor::White);
        connection_state.drive_game_start = event.drive_game_start;

        info!("Hosting game {}. Waiting for peer to connect...", game_id);
        info!("Share your Node ID: {:?}", network_state.node_id);
    }
}

fn handle_connect_to_peer(
    mut events: MessageReader<ConnectToPeerEvent>,
    mut connection_state: ResMut<P2PConnectionState>,
    network_state: Res<OnlineNetworkState>,
    player_identity: Res<crate::states::main_menu::PlayerIdentity>,
) {
    for event in events.read() {
        // Guard: don't re-connect if already connecting/connected/in-game
        match &connection_state.status {
            P2PConnectionStatus::Connecting
            | P2PConnectionStatus::Connected
            | P2PConnectionStatus::InGame => {
                info!(
                    "Ignoring duplicate connect request — already {:?}",
                    connection_state.status
                );
                continue;
            }
            _ => {}
        }

        let game_id: u64 = rand::random();

        connection_state.local_node_id = network_state.node_id.clone();
        connection_state.peer_node_id = Some(event.peer_node_id.clone());
        connection_state.game_id = Some(game_id);
        connection_state.is_host = event.is_host;
        connection_state.status = P2PConnectionStatus::Connecting;
        connection_state.connecting_since = Some(std::time::Instant::now());
        connection_state.drive_game_start = event.drive_game_start;
        connection_state.player_color = Some(if event.is_host {
            PieceColor::White
        } else {
            PieceColor::Black
        });

        // Decode peer ID from bs58
        let peer_endpoint_id = match bs58::decode(&event.peer_node_id).into_vec() {
            Ok(decoded) => {
                if decoded.len() == 32 {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(&decoded);
                    match iroh::EndpointId::from_bytes(&bytes) {
                        Ok(id) => Some(id),
                        Err(e) => {
                            error!("Invalid Node ID bytes: {}", e);
                            connection_state.status =
                                P2PConnectionStatus::Error("Invalid Node ID bytes".to_string());
                            None
                        }
                    }
                } else {
                    error!("Node ID wrong length: expected 32, got {}", decoded.len());
                    connection_state.status =
                        P2PConnectionStatus::Error("Node ID wrong length".to_string());
                    None
                }
            }
            Err(e) => {
                error!("Invalid base58 Node ID: {}", e);
                connection_state.status =
                    P2PConnectionStatus::Error("Invalid base58 Node ID".to_string());
                None
            }
        };

        if let Some(endpoint_id) = peer_endpoint_id {
            if network_state.connected_peers.contains(&event.peer_node_id) {
                // Already connected, send invite immediately
                info!(
                    "Already connected to {}, sending invite directly",
                    event.peer_node_id
                );
                if let Some(tx) = &network_state.message_sender {
                    let our_node_id = network_state
                        .node_id
                        .as_ref()
                        .map(|id| bs58::encode(id.as_bytes()).into_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    let invite = NetworkMessage::GameInvite {
                        game_id,
                        from_node: our_node_id,
                        from_wallet: "pvp_player".to_string(), // Simplified for PvP
                        from_display: player_identity.display_name().to_string(),
                    };

                    if let Err(e) = tx.send(invite) {
                        error!("Failed to send game invite: {}", e);
                        connection_state.status =
                            P2PConnectionStatus::Error(format!("Failed to send invite: {}", e));
                    } else {
                        info!(
                            "Sent game invite {} to peer {}",
                            game_id, event.peer_node_id
                        );
                    }
                }
            } else {
                if let Some(bootstrap_tx) = &network_state.bootstrap_sender {
                    if let Err(e) = bootstrap_tx.send(endpoint_id) {
                        error!("Failed to send bootstrap peer: {}", e);
                        connection_state.status =
                            P2PConnectionStatus::Error(format!("Failed to bootstrap: {}", e));
                    } else {
                        info!(
                            "Initiated connection to peer node {}. Waiting for network...",
                            event.peer_node_id
                        );
                    }
                } else {
                    error!("Network not initialized - cannot connect to peer");
                    connection_state.status =
                        P2PConnectionStatus::Error("Network not initialized".to_string());
                }
            }
        }
    }
}

fn handle_network_events(
    mut network_events: MessageReader<NetworkEvent>,
    mut connection_state: ResMut<P2PConnectionState>,
    mut network_state: ResMut<OnlineNetworkState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut game_started: MessageWriter<GameStartedEvent>,
    mut core_mode: ResMut<crate::core::GameMode>,
    mut ai_config: ResMut<crate::game::ai::ChessAIResource>,
    player_identity: Res<crate::states::main_menu::PlayerIdentity>,
    #[cfg(feature = "solana")] mut solana_sync: Option<
        ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
    >,
    #[cfg(feature = "solana")] mut competitive: Option<
        ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    >,
) {
    for event in network_events.read() {
        match event {
            NetworkEvent::PeerConnected(peer_id) => {
                info!("PeerConnected: {}", peer_id);
                network_state.connected_peers.insert(peer_id.clone());
                // If we are connecting and this is the peer we want to connect to
                if let P2PConnectionStatus::Connecting = connection_state.status {
                    if Some(peer_id.clone()) == connection_state.peer_node_id {
                        let game_id = connection_state.game_id.unwrap_or_else(rand::random);
                        connection_state.game_id = Some(game_id);

                        if let Some(tx) = &network_state.message_sender {
                            let our_node_id = network_state
                                .node_id
                                .as_ref()
                                .map(|id| bs58::encode(id.as_bytes()).into_string())
                                .unwrap_or_else(|| "unknown".to_string());

                            let invite = NetworkMessage::GameInvite {
                                game_id,
                                from_node: our_node_id,
                                from_wallet: "pvp_player".to_string(), // Simplified for PvP
                                from_display: player_identity.display_name().to_string(),
                            };

                            if let Err(e) = tx.send(invite) {
                                error!("Failed to send game invite: {}", e);
                                connection_state.status = P2PConnectionStatus::Error(format!(
                                    "Failed to send invite: {}",
                                    e
                                ));
                            } else {
                                info!(
                                    "Sent game invite {} to peer {} after network connect",
                                    game_id, peer_id
                                );

                                // Subscribe to game-specific topic for move traffic
                                if let Some(sub_tx) = &network_state.subscription_sender {
                                    let _ = sub_tx.send(format!(
                                        "{}/{}",
                                        crate::multiplayer::systems::GAME_TOPIC,
                                        game_id
                                    ));
                                }
                            }
                        } else {
                            error!("Network not initialized - cannot send invite");
                            connection_state.status =
                                P2PConnectionStatus::Error("Network not initialized".to_string());
                        }
                    }
                }
            }
            NetworkEvent::MessageReceived(msg) => {
                match msg {
                    NetworkMessage::GameInvite {
                        game_id,
                        from_node,
                        from_display,
                        ..
                    } => {
                        info!(
                            "Received game invite from {} ({}) for game {}",
                            from_node, from_display, game_id
                        );

                        if connection_state.is_host {
                            info!(
                                "Peer joined our hosted game! Auto-accepting invite from {}",
                                from_node
                            );
                            connection_state.peer_node_id = Some(from_node.clone());
                            connection_state.game_id = Some(*game_id);
                            connection_state.status = P2PConnectionStatus::Connected;
                            if !from_display.is_empty() {
                                connection_state.opponent_display_name = Some(from_display.clone());
                            }

                            if let Some(tx) = &network_state.message_sender {
                                let response = NetworkMessage::InviteResponse {
                                    game_id: *game_id,
                                    accepted: true,
                                    display_name: player_identity.display_name().to_string(),
                                };
                                if let Err(e) = tx.send(response) {
                                    error!("Failed to send invite acceptance: {}", e);
                                    connection_state.status = P2PConnectionStatus::Error(format!(
                                        "Failed to accept invite: {}",
                                        e
                                    ));
                                } else {
                                    info!("Sent InviteResponse(accepted) for game {}", game_id);
                                }
                            } else {
                                error!("Network not initialized - cannot accept invite");
                                connection_state.status = P2PConnectionStatus::Error(
                                    "Network not initialized".to_string(),
                                );
                            }
                        } else {
                            connection_state.peer_node_id = Some(from_node.clone());
                            connection_state.game_id = Some(*game_id);
                            connection_state.status = P2PConnectionStatus::Connected;
                            connection_state.player_color = Some(PieceColor::Black);
                            if !from_display.is_empty() {
                                connection_state.opponent_display_name = Some(from_display.clone());
                            }

                            // Subscribe to game-specific topic for move traffic
                            if let Some(sub_tx) = &network_state.subscription_sender {
                                let _ = sub_tx.send(format!(
                                    "{}/{}",
                                    crate::multiplayer::systems::GAME_TOPIC,
                                    game_id
                                ));
                            }
                        }
                    }

                    NetworkMessage::InviteResponse {
                        game_id,
                        accepted,
                        display_name,
                    } => {
                        if connection_state.game_id == Some(*game_id) {
                            if *accepted {
                                info!("Host accepted invite for game {}", game_id);
                                if !display_name.is_empty() {
                                    connection_state.opponent_display_name =
                                        Some(display_name.clone());
                                }

                                if let Some(tx) = &network_state.message_sender {
                                    let local_str = connection_state
                                        .local_node_id
                                        .as_ref()
                                        .or(network_state.node_id.as_ref())
                                        .map(|id| bs58::encode(id.as_bytes()).into_string())
                                        .unwrap_or_default();
                                    let peer_str =
                                        connection_state.peer_node_id.clone().unwrap_or_default();

                                    // Host (peer) is White; joiner (us) is Black
                                    let white_player = peer_str.clone();
                                    let black_player = local_str.clone();
                                    let my_display = player_identity.display_name().to_string();

                                    let start_msg = NetworkMessage::GameStart {
                                        game_id: *game_id,
                                        white_player: white_player.clone(),
                                        black_player: black_player.clone(),
                                        initial_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
                                        white_display: display_name.clone(),
                                        black_display: my_display,
                                    };

                                    let _ = tx.send(start_msg);

                                    if connection_state.drive_game_start {
                                        // This is the *only* signaling channel Direct
                                        // Connection / join-links have — drive the
                                        // transition ourselves and make sure the game
                                        // actually comes up in online-multiplayer mode
                                        // (previously nothing set these, so both of
                                        // those flows silently landed in
                                        // GameMode::SinglePlayer / ChessAIResource's
                                        // default VsAI instead).
                                        connection_state.status = P2PConnectionStatus::InGame;
                                        *core_mode = crate::core::GameMode::OnlineMultiplayer;
                                        ai_config.mode =
                                            crate::game::ai::resource::GameMode::Multiplayer;
                                        // Pure casual P2P entry: clear any stale on-chain
                                        // game context from a previous Solana lobby /
                                        // tournament match (see
                                        // `clear_on_chain_game_state`).
                                        #[cfg(feature = "solana")]
                                        crate::multiplayer::solana::addon::clear_on_chain_game_state(
                                            solana_sync.as_deref_mut(),
                                            competitive.as_deref_mut(),
                                        );
                                        // Joiner won't receive its own gossip broadcast,
                                        // so transition to InGame directly here.
                                        game_started.write(GameStartedEvent { game_id: *game_id });
                                        next_state.set(GameState::InGame);
                                    } else {
                                        // VPS-lobby / tournament flow: this is just the
                                        // opportunistic dual-transport link coming up.
                                        // Game-start is owned by the host's explicit
                                        // "Start Game" (relayed GAME_START) or by
                                        // handle_tournament_match_assigned — don't race it.
                                        connection_state.status = P2PConnectionStatus::Connected;
                                        info!(
                                            "Direct link ready for game {} (non-authoritative — waiting for the real game-start signal)",
                                            game_id
                                        );
                                    }
                                }
                            } else {
                                info!("Host rejected invite for game {}", game_id);
                                connection_state.status = P2PConnectionStatus::Disconnected;
                                connection_state.game_id = None;
                            }
                        }
                    }

                    NetworkMessage::GameStart {
                        game_id,
                        white_player,
                        black_player,
                        initial_fen: _,
                        white_display,
                        black_display,
                    } => {
                        if connection_state.game_id == Some(*game_id) {
                            if connection_state.drive_game_start {
                                info!(
                                    "Game {} started! White: {}, Black: {}",
                                    game_id, white_player, black_player
                                );
                                // The opponent's display name for our own color was
                                // already captured earlier in the handshake (from
                                // GameInvite/InviteResponse); this message is the
                                // first point the *other* side's name arrives, so
                                // fill it in here too if it's still unset.
                                if connection_state.opponent_display_name.is_none() {
                                    let opponent_name = match connection_state.player_color {
                                        Some(PieceColor::White) => black_display.clone(),
                                        _ => white_display.clone(),
                                    };
                                    if !opponent_name.is_empty() {
                                        connection_state.opponent_display_name =
                                            Some(opponent_name);
                                    }
                                }
                                connection_state.status = P2PConnectionStatus::InGame;
                                *core_mode = crate::core::GameMode::OnlineMultiplayer;
                                ai_config.mode = crate::game::ai::resource::GameMode::Multiplayer;

                                // Pure casual P2P entry: clear any stale on-chain
                                // game context from a previous Solana lobby /
                                // tournament match (see
                                // `clear_on_chain_game_state`).
                                #[cfg(feature = "solana")]
                                crate::multiplayer::solana::addon::clear_on_chain_game_state(
                                    solana_sync.as_deref_mut(),
                                    competitive.as_deref_mut(),
                                );

                                game_started.write(GameStartedEvent { game_id: *game_id });

                                next_state.set(GameState::InGame);
                            } else {
                                connection_state.status = P2PConnectionStatus::Connected;
                                info!(
                                    "Direct link ready for game {} (non-authoritative — waiting for the real game-start signal)",
                                    game_id
                                );
                            }
                        }
                    }

                    _ => {}
                }
            }
            NetworkEvent::PeerDisconnected(reason) => {
                // Only meaningful mid-game: during lobby setup a dropped/failed
                // link is already surfaced via explicit Error(...) transitions.
                if connection_state.status == P2PConnectionStatus::InGame {
                    warn!("PeerDisconnected during game: {}", reason);
                    connection_state.status = P2PConnectionStatus::Disconnected;
                }
            }
            _ => {}
        }
    }
}

fn handle_accept_invite(
    mut events: MessageReader<AcceptInviteEvent>,
    connection_state: ResMut<P2PConnectionState>,
    network_state: Res<OnlineNetworkState>,
    player_identity: Res<crate::states::main_menu::PlayerIdentity>,
) {
    for event in events.read() {
        if connection_state.game_id == Some(event.game_id) {
            if let Some(tx) = &network_state.message_sender {
                let response = NetworkMessage::InviteResponse {
                    game_id: event.game_id,
                    accepted: true,
                    display_name: player_identity.display_name().to_string(),
                };

                if let Err(e) = tx.send(response) {
                    error!("Failed to send invite acceptance: {}", e);
                } else {
                    info!("Accepted invite for game {}", event.game_id);
                }
            }
        }
    }
}

fn tick_connection_timeout(mut connection_state: ResMut<P2PConnectionState>) {
    if let P2PConnectionStatus::Connecting = &connection_state.status {
        if let Some(since) = connection_state.connecting_since {
            if since.elapsed().as_secs() >= 12 {
                warn!("[P2P] Connection timed out after 12s — resetting to allow retry");
                connection_state.status =
                    P2PConnectionStatus::Error("Connection timed out — try again".to_string());
                connection_state.connecting_since = None;
            }
        }
    }
}

fn handle_reject_invite(
    mut events: MessageReader<RejectInviteEvent>,
    mut connection_state: ResMut<P2PConnectionState>,
    network_state: Res<OnlineNetworkState>,
) {
    for event in events.read() {
        if connection_state.game_id == Some(event.game_id) {
            if let Some(tx) = &network_state.message_sender {
                let response = NetworkMessage::InviteResponse {
                    game_id: event.game_id,
                    accepted: false,
                    display_name: String::new(),
                };

                if let Err(e) = tx.send(response) {
                    error!("Failed to send invite rejection: {}", e);
                } else {
                    info!("Rejected invite for game {}", event.game_id);
                }
            }

            // Reset connection state
            connection_state.status = P2PConnectionStatus::Disconnected;
            connection_state.game_id = None;
            connection_state.peer_node_id = None;
        }
    }
}
