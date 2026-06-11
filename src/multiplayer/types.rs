use bevy::prelude::*;
use iroh::EndpointId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use crate::multiplayer::traits::Message;
use crate::multiplayer::network::protocol::NetworkMessage;

/// Role of a node in the gossip network
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum NodeRole {
    /// Active game participant
    Player,
    /// Read-only observer
    Spectator,
    /// TURN relay node
    Relay,
    /// Tournament official/oracle
    Arbiter,
}

#[derive(Debug, Clone, Serialize, Deserialize, Reflect)]
pub struct TimeControl {
    pub base_time_seconds: u32,
    pub increment_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Reflect)]
pub enum ChessVariant {
    Standard,
    Chess960,
    ThreeCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize, Reflect)]
pub struct GamePreferences {
    pub stake_amount: f64,
    pub time_control: TimeControl,
    pub variant: ChessVariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum PlayerColor {
    White,
    Black,
}

#[derive(Debug, Clone, Reflect)]
pub struct PeerInfo {
    pub node_id: String,
    pub wallet_address: String,
    pub game_preferences: GamePreferences,
    pub last_seen: Instant,
    pub role: NodeRole,
    pub connected_game: Option<u64>,
}

#[derive(Debug, Clone, Reflect)]
pub struct MultiplayerGameState {
    pub game_id: u64,
    pub my_color: PlayerColor,
    pub initial_fen: String,
    pub last_active: Instant,
}

#[derive(Debug, Clone, Reflect)]
pub struct GameSession {
    pub session_id: String,
    pub opponent: PeerInfo,
    pub stake_amount: f64,
    pub confirmed: bool,
    pub started: bool,
    pub game_state: Option<MultiplayerGameState>,
}

/// Tracks the causal version chain for each active game.
///
/// Used to detect equivocation (two moves claiming the same parent) and
/// sequence gaps (a move whose `seq` is not `last_seq + 1`).
/// Updated in `handle_network_events` before moves are forwarded to the board.
#[derive(Resource, Default)]
pub struct CausalChainState {
    /// (game_id, agent_id_bytes) → last accepted sequence number from that agent.
    pub last_seq: HashMap<(u64, Vec<u8>), u64>,
    /// (game_id, agent_id_bytes) → version_hash of that agent's last accepted move.
    ///
    /// Gap B: the head is tracked PER (game, agent), not per game. With a single
    /// shared slot, a move from one identity could overwrite the head another
    /// identity's chain is validated against. Per-sender lanes mean an injected
    /// move from a stranger cannot poison the real opponent's causal chain.
    pub head_version: HashMap<(u64, Vec<u8>), String>,
    /// game_id → allowed signer pubkeys (the participants' session keys).
    ///
    /// A2: populated from `SessionInfo` (sent only after the VPS confirms a
    /// session is active). Once non-empty, a `Move` whose verified signer
    /// (`agent_id`, bound in `bind_identity`) is not listed is rejected before
    /// it can reach the board. Capped at two — the two players.
    pub roster: HashMap<u64, Vec<Vec<u8>>>,
}

#[derive(Resource)]
pub struct BraidNetworkState {
    pub node_id: Option<EndpointId>,
    pub secret_key_bytes: Option<[u8; 32]>,
    pub connected: bool,
    pub discovered_peers: Vec<PeerInfo>,
    pub connected_peers: std::collections::HashSet<String>,
    pub active_session: Option<GameSession>,
    pub pending_invites: HashMap<String, GamePreferences>,
    pub event_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<NetworkEvent>>,
    pub message_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkMessage>>,
    pub bootstrap_sender: Option<tokio::sync::mpsc::UnboundedSender<EndpointId>>,
    pub subscription_sender: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    /// Raw 32-byte Ed25519 signing key for the on-chain session.
    /// When present, all outgoing [`NetworkMessage`]s are signed before broadcast.
    pub session_signing_key: Option<[u8; 32]>,
    /// Per-game expected nonce for P2P-layer replay protection.
    /// Moves with nonce < expected are dropped; nonce >= expected are accepted
    /// and the map is updated to nonce + 1.
    pub expected_nonces: std::collections::HashMap<u64, u64>,
}

impl Default for BraidNetworkState {
    fn default() -> Self {
        Self {
            node_id: None,
            secret_key_bytes: None,
            connected: false,
            discovered_peers: Vec::new(),
            connected_peers: std::collections::HashSet::new(),
            active_session: None,
            pending_invites: HashMap::new(),
            event_receiver: None,
            message_sender: None,
            bootstrap_sender: None,
            subscription_sender: None,
            session_signing_key: None,
            expected_nonces: std::collections::HashMap::new(),
        }
    }
}

#[derive(Resource, Default)]
pub struct BraidGameSync {
    pub pending_patches: Vec<Vec<u8>>,
}

/// Tracks heartbeat timing for the active P2P game session.
#[derive(Resource)]
pub struct HeartbeatState {
    /// Elapsed seconds since the last Ping was sent.
    pub since_last_ping: f32,
    /// Elapsed seconds since the last Pong was received from the opponent.
    pub since_last_pong: f32,
    /// How often (seconds) to send a Ping.
    pub ping_interval: f32,
    /// How long (seconds) without a Pong before declaring the opponent disconnected.
    pub timeout_secs: f32,
    /// Whether the opponent has been declared disconnected this session.
    pub timed_out: bool,
}

impl Default for HeartbeatState {
    fn default() -> Self {
        Self {
            since_last_ping: 0.0,
            since_last_pong: 0.0,
            ping_interval: 5.0,
            timeout_secs: 15.0,
            timed_out: false,
        }
    }
}

#[derive(Event, Message, Debug, Clone)]
pub enum NetworkEvent {
    NetworkInitialized {
        node_id: EndpointId,
        secret_key_bytes: [u8; 32],
    },
    PeerDiscovered(PeerInfo),
    GameInviteReceived(String, GamePreferences),
    GameInviteAccepted(String),
    WagerHandshake {
        node_id: String,
        game_id: u64,
    },
    MessageReceived(NetworkMessage),
    GameEnded(String),
    PeerConnected(String),
    PeerDisconnected(String),
    /// A move was rejected because signature verification failed, nonce was invalid,
    /// or the move failed engine validation.
    InvalidMoveRejected {
        game_id: u64,
        reason: String,
    },
}

/// Central configuration for multiplayer backend URLs.
#[derive(Resource, Debug, Clone)]
pub struct NetworkConfig {
    pub vps_base_url: String,
    pub relay_base_url: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            vps_base_url: "http://178.104.55.19".to_string(), // Unified production IP
            relay_base_url: "http://178.104.55.19".to_string(),
        }
    }
}
