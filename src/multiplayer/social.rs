//! Bevy social subsystem: friends list, presence, lobby invites, lobby chat.
//!
//! Design: node ID is the stable identity; Solana pubkey is optional.
//! Friends survive wallet rotation because the contact graph is node-ID anchored.

use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::time::Instant;
use tracing::info;

use crate::multiplayer::network::vps::{
    get_contacts, get_online, get_pending_requests, poll_social, update_presence, LobbyInvite,
    SocialContact, SocialFriendRequest, SocialPresence,
};
use crate::multiplayer::types::NetworkEvent;

// ── Resources ────────────────────────────────────────────────────────────────

/// Bevy resource holding the full social state for the local player.
#[derive(Resource)]
pub struct FriendsState {
    pub contacts: Vec<SocialContact>,
    pub pending_requests: Vec<SocialFriendRequest>,
    pub pending_invites: Vec<LobbyInvite>,
    pub social_poll_index: usize,
    pub loading: bool,
    pub last_sync: Option<Instant>,
    /// Our own node ID (populated by the network layer after Iroh node spawns)
    pub our_node_id: Option<String>,
    /// Our Solana pubkey (populated when wallet is connected)
    pub our_pubkey: Option<String>,
    /// Our display name
    pub our_display: String,
    /// Background fetch receiver
    pub fetch_rx: Option<Receiver<FriendsFetchResult>>,
}

#[derive(Debug)]
pub struct FriendsFetchResult {
    pub contacts: Vec<SocialContact>,
    pub pending_requests: Vec<SocialFriendRequest>,
    pub invites: Vec<LobbyInvite>,
    pub next_poll_index: usize,
}

impl Default for FriendsState {
    fn default() -> Self {
        Self {
            contacts: Vec::new(),
            pending_requests: Vec::new(),
            pending_invites: Vec::new(),
            social_poll_index: 0,
            loading: false,
            last_sync: None,
            our_node_id: None,
            our_pubkey: None,
            our_display: "Anonymous".to_string(),
            fetch_rx: None,
        }
    }
}

/// Bevy resource holding the count of players currently online (per the VPS
/// presence store). Refreshed every ~15s by [`tick_presence_sync`].
#[derive(Resource, Default)]
pub struct OnlinePlayersState {
    /// Number of players the backend reports as online (Online + InGame).
    pub count: usize,
    pub last_sync: Option<Instant>,
    /// Background fetch receiver for the `GET /presence` result.
    pub fetch_rx: Option<Receiver<usize>>,
}

/// Bevy resource tracking the backend region + measured latency to it.
#[derive(Resource, Default)]
pub struct BackendRegion {
    pub tag: String,
    pub label: String,
    pub latency_ms: Option<u32>,
}

// ── Lobby chat ───────────────────────────────────────────────────────────────

/// Pre-game lobby chat — active from lobby creation until game start.
/// Backed by the same Braid-HTTP `/chat/:game_id` resource as in-game chat.
#[derive(Resource)]
pub struct LobbyChatSession {
    pub game_id: String,
    pub active: bool,
    pub messages: Vec<LobbyMsg>,
    pub draft: String,
    /// Inbound chat receiver from Braid-HTTP subscriber task
    pub rx: Option<Receiver<LobbyMsg>>,
    /// Outbound sender handed to background task
    pub tx: Sender<LobbyMsg>,
    // internal only — the other half stored so we don't drop it
    _tx_keep: Sender<LobbyMsg>,
}

#[derive(Debug, Clone)]
pub struct LobbyMsg {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
}

impl Default for LobbyChatSession {
    fn default() -> Self {
        let (tx, _) = bounded::<LobbyMsg>(64);
        let (tx2, rx) = bounded::<LobbyMsg>(64);
        Self {
            game_id: String::new(),
            active: false,
            messages: Vec::new(),
            draft: String::new(),
            rx: Some(rx),
            tx: tx2.clone(),
            _tx_keep: tx,
        }
    }
}

impl LobbyChatSession {
    pub fn activate(&mut self, game_id: String, base_url: String, display: String) {
        if self.active && self.game_id == game_id {
            return;
        }

        let (tx_in, rx_in) = bounded::<LobbyMsg>(64);
        let (tx_out, _rx_out) = bounded::<LobbyMsg>(64);

        self.game_id = game_id.clone();
        self.active = true;
        self.messages.clear();
        self.rx = Some(rx_in);
        self.tx = tx_out.clone();
        self._tx_keep = tx_out;

        // Spawn a Braid-HTTP chat subscriber in background
        let sender = tx_in;
        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                let sub = match braid_chess::ChessSubscriber::new(&base_url, &game_id) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("[lobby-chat] subscriber init failed: {e}");
                        return;
                    }
                };
                let (rx, _handle) = match sub.subscribe_chat().await {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!("[lobby-chat] subscribe_chat failed: {e}");
                        return;
                    }
                };
                tracing::info!("[lobby-chat] Subscribed to {} @ {}", game_id, base_url);
                while let Ok(msg) = rx.recv().await {
                    if let braid_chess::ChessMessage::Chat(p) = msg {
                        let lobby_msg = LobbyMsg {
                            player: display.clone(),
                            text: p.text,
                            timestamp_ms: p.timestamp_ms,
                        };
                        if sender.send(lobby_msg).is_err() {
                            break;
                        }
                    }
                }
            })
            .detach();
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.rx = None;
    }
}

// ── Lobby filter config ───────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct LobbyFilterConfig {
    pub time_min: Option<u32>,
    pub time_max: Option<u32>,
    pub stake_min: Option<f64>,
    pub stake_max: Option<f64>,
    pub elo_min: Option<u16>,
    pub elo_max: Option<u16>,
    pub sort: LobbySort,
    pub dirty: bool, // set to true when filters change to force re-fetch
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum LobbySort {
    #[default]
    Newest,
    EloAsc,
    EloDesc,
    StakeAsc,
    StakeDesc,
    TimeAsc,
}

impl LobbySort {
    pub fn as_str(&self) -> &'static str {
        match self {
            LobbySort::Newest => "newest",
            LobbySort::EloAsc => "elo_asc",
            LobbySort::EloDesc => "elo_desc",
            LobbySort::StakeAsc => "stake_asc",
            LobbySort::StakeDesc => "stake_desc",
            LobbySort::TimeAsc => "time_asc",
        }
    }
}

// ── Lobby fetch state ─────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct LobbyFetchState {
    pub status: LobbyLoadStatus,
    pub games: Vec<crate::multiplayer::vps_client::P2PGameListing>,
    pub last_fetch: Option<Instant>,
    pub rx: Option<Receiver<Result<Vec<crate::multiplayer::vps_client::P2PGameListing>, String>>>,
}

#[derive(Default, PartialEq, Clone)]
pub enum LobbyLoadStatus {
    #[default]
    Idle,
    Fetching,
    Done,
    Error(String),
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct SocialPlugin;

impl Plugin for SocialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FriendsState>()
            .init_resource::<LobbyChatSession>()
            .init_resource::<LobbyFilterConfig>()
            .init_resource::<LobbyFetchState>()
            .init_resource::<BackendRegion>()
            .init_resource::<OnlinePlayersState>()
            .add_systems(
                Update,
                (
                    sync_node_id_from_network,
                    poll_friends_fetch,
                    drain_lobby_chat,
                    tick_friends_sync,
                    tick_presence_sync,
                    fetch_backend_region_once,
                ),
            );
    }
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// Copy the node ID into FriendsState as soon as the Iroh node finishes initializing.
fn sync_node_id_from_network(
    mut friends: ResMut<FriendsState>,
    mut events: crate::multiplayer::traits::MessageReader<NetworkEvent>,
) {
    for event in events.read() {
        if let NetworkEvent::NetworkInitialized { node_id, .. } = event {
            let id_str = bs58::encode(node_id.as_bytes()).into_string();
            if friends.our_node_id.as_deref() != Some(&id_str) {
                info!("[social] Node ID set: {}", id_str);
                friends.our_node_id = Some(id_str);
            }
        }
    }
}

/// Poll the oneshot receiver for friends data and write it into FriendsState.
fn poll_friends_fetch(mut state: ResMut<FriendsState>) {
    let result = if let Some(ref rx) = state.fetch_rx {
        rx.try_recv().ok()
    } else {
        return;
    };

    if let Some(result) = result {
        state.contacts = result.contacts;
        state.pending_requests = result.pending_requests;
        // Append new invites (don't duplicate)
        for inv in result.invites {
            if !state
                .pending_invites
                .iter()
                .any(|e| e.game_id == inv.game_id && e.from_node_id == inv.from_node_id)
            {
                state.pending_invites.push(inv);
            }
        }
        state.social_poll_index = result.next_poll_index;
        state.loading = false;
        state.last_sync = Some(Instant::now());
        state.fetch_rx = None;
    }
}

/// Drain inbound lobby chat messages into LobbyChatSession.messages.
fn drain_lobby_chat(mut chat: ResMut<LobbyChatSession>) {
    if !chat.active {
        return;
    }
    let msgs: Vec<LobbyMsg> = if let Some(ref rx) = chat.rx {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    } else {
        return;
    };
    chat.messages.extend(msgs);
}

/// Every 15 seconds, kick off a background friends + social poll if our node_id is known.
fn tick_friends_sync(mut state: ResMut<FriendsState>) {
    if state.loading {
        return;
    }
    if state.fetch_rx.is_some() {
        return;
    }
    let Some(ref node_id) = state.our_node_id.clone() else {
        return;
    };

    let elapsed = state
        .last_sync
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(u64::MAX);
    if elapsed < 15 {
        return;
    }

    state.loading = true;

    let (tx, rx) = bounded(1);
    state.fetch_rx = Some(rx);

    let node_id = node_id.clone();
    let pubkey = state.our_pubkey.clone();
    let poll_index = state.social_poll_index;

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let contacts = get_contacts(&node_id, pubkey.as_deref()).unwrap_or_default();
            let pending_requests =
                get_pending_requests(&node_id, pubkey.as_deref()).unwrap_or_default();
            let poll_resp = poll_social(&node_id, poll_index).unwrap_or_else(|_| {
                crate::multiplayer::vps_client::SocialPollResponse {
                    invites: vec![],
                    next_index: poll_index,
                }
            });
            let _ = tx.send(FriendsFetchResult {
                contacts,
                pending_requests,
                invites: poll_resp.invites,
                next_poll_index: poll_resp.next_index,
            });
        })
        .detach();
}

/// Every ~15s: send our presence heartbeat (`PUT /presence`) so we count as
/// online, then fetch the current online count (`GET /presence`). Both run on a
/// background IO task; the count is drained into [`OnlinePlayersState`].
fn tick_presence_sync(friends: Res<FriendsState>, mut online: ResMut<OnlinePlayersState>) {
    // Drain any in-flight result first.
    if let Some(rx) = online.fetch_rx.as_ref() {
        if let Ok(count) = rx.try_recv() {
            online.count = count;
            online.last_sync = Some(Instant::now());
            online.fetch_rx = None;
        }
    }

    if online.fetch_rx.is_some() {
        return;
    }
    let elapsed = online
        .last_sync
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(u64::MAX);
    if elapsed < 15 {
        return;
    }

    // Need a stable identity before we can announce presence.
    let Some(node_id) = friends.our_node_id.clone() else {
        return;
    };
    let pubkey = friends.our_pubkey.clone();
    let display = friends.our_display.clone();

    let (tx, rx) = bounded(1);
    online.fetch_rx = Some(rx);

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            // Heartbeat — best-effort; ignore failures.
            let _ = update_presence(&SocialPresence {
                node_id,
                pubkey,
                display_name: display,
                status: "online".to_string(),
                game_id: None,
                updated_at: chrono::Utc::now().to_rfc3339(),
            });
            let count = get_online().map(|v| v.len()).unwrap_or(0);
            let _ = tx.send(count);
        })
        .detach();
}

/// One-shot region fetch — spawns a background task on first run, then drains
/// the channel every frame until a value arrives.
fn fetch_backend_region_once(
    mut region: ResMut<BackendRegion>,
    mut state: Local<Option<Receiver<(String, String, u32)>>>,
) {
    if state.is_none() {
        let (tx, rx) = bounded::<(String, String, u32)>(1);
        *state = Some(rx);
        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                let start = std::time::Instant::now();
                let (tag, label) = crate::multiplayer::vps_client::fetch_region()
                    .unwrap_or_else(|_| ("unknown".to_string(), "Unknown Region".to_string()));
                let latency_ms = start.elapsed().as_millis() as u32;
                let _ = tx.send((tag, label, latency_ms));
            })
            .detach();
    }

    if let Some(ref rx) = *state {
        if let Ok((tag, label, latency_ms)) = rx.try_recv() {
            region.tag = tag;
            region.label = label;
            region.latency_ms = Some(latency_ms);
            *state = None;
        }
    }
}
