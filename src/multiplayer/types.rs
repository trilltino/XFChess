use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::traits::Message;
use bevy::prelude::*;
use iroh::EndpointId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

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
    /// (`signer_pubkey`, bound in `bind_identity`) is not listed is rejected before
    /// it can reach the board. Capped at two — the two players.
    pub roster: HashMap<u64, Vec<Vec<u8>>>,
    /// game_id → set of move `version_hash`es already applied to the board,
    /// from *either* transport.
    ///
    /// Gossip and the Braid moves log (`crate::multiplayer::network::braid_transport`)
    /// are two independent delivery paths for the same move. Gossip messages
    /// carry a P2P `nonce`/`seq` the `NonceSequencer`/per-agent `head_version`
    /// checks above can dedupe against; Braid's `ChessMessage::Move` payload
    /// carries neither field (it's a different wire type, shared with
    /// chat/clock/engine), so a move delivered by both paths would double-
    /// apply to the board without this: whichever path arrives first inserts
    /// the move's `version_hash` here, and the second arrival — regardless of
    /// which transport it came from — is recognized as the same move and
    /// skipped before it reaches `dispatch_remote_moves`.
    pub applied_versions: HashMap<u64, std::collections::HashSet<String>>,
    /// Braid versions emitted toward the board but not confirmed applied yet.
    pub pending_versions: HashMap<u64, std::collections::HashSet<String>>,
    /// game_id → backend-verified `(white_wallet_b58, black_wallet_b58)`,
    /// fetched once at game start
    /// (`multiplayer::solana::integration::systems::spawn_verified_participants_fetch`,
    /// `--features solana` only — always empty otherwise).
    ///
    /// Phase C (`docs/plans/networking-hardening-plan.md`): closes the
    /// roster-building race on the gossip side, mirroring Phase B's backend
    /// fix. Before a `SessionInfo`'s `signing_pubkey` is trusted into
    /// `roster` below, its accompanying `player_pubkey` is checked against
    /// this map — a forged claim that wins the arrival-order race still
    /// can't seat an attacker's signing key once this is populated. Absent
    /// (casual games, which never resolve to an on-chain `Game` account; or
    /// the narrow window before the fetch completes) falls back to the
    /// original first-two-seen trust — same fail-open-when-unavailable
    /// shape as `game_log.rs`'s `OnChainCheck::Unavailable` on the backend.
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
    /// Clone of the event channel sender, so alternate transports can inject
    /// `NetworkEvent::MessageReceived` into the same pipeline gossip feeds.
    pub event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
    pub message_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkMessage>>,
    pub bootstrap_sender: Option<tokio::sync::mpsc::UnboundedSender<EndpointId>>,
    pub subscription_sender: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    /// Raw 32-byte Ed25519 signing key for the on-chain session.
    /// When present, all outgoing [`NetworkMessage`]s are signed before broadcast.
    ///
    /// Mirrored into `session_signing_key_shared` below for the actual
    /// outgoing-message task to read — that task is spawned once at startup
    /// (`initialize_braid_network`, well before any wallet connects) and
    /// only ever sees a plain `Option<[u8;32]>` copy taken at spawn time, so
    /// writing only this ECS-side field would never reach it. This field
    /// itself is kept for other (synchronous, ECS-side) readers/logging.
    pub session_signing_key: Option<[u8; 32]>,
    /// Shared cell the outgoing-message tokio task reads fresh on every send,
    /// so a signing key that becomes available *after* that task was spawned
    /// (i.e. every real run — wallet connect always happens after app boot)
    /// actually gets used instead of the task forever seeing the `None` it
    /// captured at spawn time. See `session_signing_key`'s doc comment.
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

/// Coordinates the local game start across both online participants.
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

/// Per-game nonce reordering buffer for the dual gossip+relay transport.
///
/// See `crate::multiplayer::network::reorder` for why this exists: gossip
/// and the VPS relay have independent, uncorrelated latency, so move N+1 can
/// legitimately arrive before move N. Each game gets its own
/// [`NonceSequencer`], created lazily on first arrival. `arrival_times`
/// tracks, per game, when the oldest currently-buffered nonce first showed
/// up — `sweep_stale_buffers` (systems.rs) uses this wall-clock age to catch
/// the case a bounded buffer alone can't: a gap with no further traffic
/// (e.g. the mover whose turn it is has nothing to send until they see the
/// move that filled it), which never grows past one buffered entry.
#[derive(Resource, Default)]
pub struct PendingMoveBuffer {
    pub sequencers:
        HashMap<u64, crate::multiplayer::network::reorder::NonceSequencer<NetworkMessage>>,
    pub oldest_buffered_since: HashMap<u64, std::time::Instant>,
}

impl PendingMoveBuffer {
    /// How long a gap may sit unfilled before `sweep_stale_buffers` forces a
    /// resync. Chess move cadence (seconds-to-minutes per move) makes a few
    /// seconds an easy call: long enough that ordinary jitter between the
    /// two transports never trips it, short enough that a genuinely dropped
    /// move doesn't stall the game for the length of a whole move.
    pub const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(5);
    /// Out-of-order arrivals held per game before giving up on the gap and
    /// forcing a resync. Real play never has more than a couple of moves in
    /// flight; this is a generous ceiling, not a tight one.
    pub const MAX_BUFFERED: usize = 8;
}

/// Tracks heartbeat timing for the active online game session.
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
    NetworkInitializationFailed(String),
    PeerDiscovered(PeerInfo),
    GameInviteReceived(String, GamePreferences),
    GameInviteAccepted(String),
    MessageReceived(NetworkMessage),
    /// A move delivered by the authenticated Braid history/live stream.
    /// Braid has version-based ordering, not a gossip nonce, so it must not
    /// pass through the gossip nonce sequencer.
    BraidMove(NetworkMessage),
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
        let base = crate::multiplayer::network::vps::vps_base();
        Self {
            vps_base_url: base.clone(),
            relay_base_url: base,
        }
    }
}
