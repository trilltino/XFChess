use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::traits::Message;
use bevy::prelude::*;
use iroh::EndpointId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum NodeRole {
    Player,
    Spectator,
    Relay,
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

#[derive(Resource, Default)]
pub struct CausalChainState {
    pub last_seq: HashMap<(u64, Vec<u8>), u64>,
    pub head_version: HashMap<(u64, Vec<u8>), String>,
    pub roster: HashMap<u64, Vec<Vec<u8>>>,
    pub applied_versions: HashMap<u64, std::collections::HashSet<String>>,
    pub pending_versions: HashMap<u64, std::collections::HashSet<String>>,
    pub verified_wallets: HashMap<u64, (String, String)>,
}

#[derive(Resource)]
pub struct OnlineNetworkState {
    pub node_id: Option<EndpointId>,
    pub secret_key_bytes: Option<[u8; 32]>,
    pub connected: bool,
    pub initialization_in_progress: bool,
    pub discovered_peers: Vec<PeerInfo>,
    pub connected_peers: std::collections::HashSet<String>,
    pub active_session: Option<GameSession>,
    pub pending_invites: HashMap<String, GamePreferences>,
    pub event_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<NetworkEvent>>,
    pub event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
    pub message_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkMessage>>,
    pub bootstrap_sender: Option<tokio::sync::mpsc::UnboundedSender<EndpointId>>,
    pub subscription_sender: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    pub session_signing_key: Option<[u8; 32]>,
    pub session_signing_key_shared: std::sync::Arc<std::sync::RwLock<Option<[u8; 32]>>>,
}

impl Default for OnlineNetworkState {
    fn default() -> Self {
        Self {
            node_id: None,
            secret_key_bytes: None,
            connected: false,
            initialization_in_progress: false,
            discovered_peers: Vec::new(),
            connected_peers: std::collections::HashSet::new(),
            active_session: None,
            pending_invites: HashMap::new(),
            event_receiver: None,
            event_sender: None,
            message_sender: None,
            bootstrap_sender: None,
            subscription_sender: None,
            session_signing_key: None,
            session_signing_key_shared: std::sync::Arc::new(std::sync::RwLock::new(None)),
        }
    }
}

#[derive(Resource, Default)]
pub struct OnlineGameSync {
    pub pending_patches: Vec<Vec<u8>>,
}

#[derive(Resource, Debug, Default)]
pub struct OnlineStartBarrier {
    pub game_id: u64,
    pub local_ready: bool,
    pub remote_ready: bool,
    pub start_confirmed: bool,
    pub ready_sent: bool,
    pub start_sent: bool,
}

impl OnlineStartBarrier {
    pub fn reset(&mut self, game_id: u64) {
        *self = Self {
            game_id,
            ..Self::default()
        };
    }

    pub fn is_complete(&self, game_id: u64) -> bool {
        self.game_id == game_id && self.local_ready && self.remote_ready && self.start_confirmed
    }
}

pub fn is_online_game_mode(mode: crate::core::GameMode) -> bool {
    matches!(
        mode,
        crate::core::GameMode::OnlineMultiplayer | crate::core::GameMode::MultiplayerCompetitive
    )
}

#[cfg(test)]
mod start_barrier_tests {
    use super::OnlineStartBarrier;

    #[test]
    fn barrier_requires_both_ready_and_start_confirmation() {
        let mut barrier = OnlineStartBarrier::default();
        barrier.reset(42);
        assert!(!barrier.is_complete(42));

        barrier.local_ready = true;
        barrier.remote_ready = true;
        assert!(!barrier.is_complete(42));

        barrier.start_confirmed = true;
        assert!(barrier.is_complete(42));
        assert!(!barrier.is_complete(43));
    }

    #[test]
    fn reset_discards_readiness_from_previous_game() {
        let mut barrier = OnlineStartBarrier {
            game_id: 42,
            local_ready: true,
            remote_ready: true,
            start_confirmed: true,
            ready_sent: true,
            start_sent: true,
        };

        barrier.reset(43);

        assert_eq!(barrier.game_id, 43);
        assert!(!barrier.local_ready);
        assert!(!barrier.remote_ready);
        assert!(!barrier.start_confirmed);
        assert!(!barrier.ready_sent);
        assert!(!barrier.start_sent);
    }
}

#[derive(Resource, Default)]
pub struct PendingMoveBuffer {
    pub sequencers:
        HashMap<u64, crate::multiplayer::network::reorder::NonceSequencer<NetworkMessage>>,
    pub oldest_buffered_since: HashMap<u64, std::time::Instant>,
}

impl PendingMoveBuffer {
    pub const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(5);
    pub const MAX_BUFFERED: usize = 8;
}

#[derive(Resource)]
pub struct HeartbeatState {
    pub since_last_ping: f32,
    pub since_last_pong: f32,
    pub ping_interval: f32,
    pub timeout_secs: f32,
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
    NetworkInitializationFailed(String),
    PeerDiscovered(PeerInfo),
    GameInviteReceived(String, GamePreferences),
    GameInviteAccepted(String),
    MessageReceived(NetworkMessage),
    BraidMove(NetworkMessage),
    GameEnded(String),
    PeerConnected(String),
    PeerDisconnected(String),
    InvalidMoveRejected {
        game_id: u64,
        reason: String,
    },
}

#[derive(Resource, Debug, Clone)]
pub struct NetworkConfig {
    pub vps_base_url: String,
    pub relay_base_url: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        let base = crate::multiplayer::network::vps::vps_base();
        Self {
            vps_base_url: base.clone(),
            relay_base_url: base,
        }
    }
}
