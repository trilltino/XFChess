//! VPS-relay transport bridge for in-game `NetworkMessage`s.
//!
//! # Why this exists
//! Moves, resigns and resync travel over **Iroh gossip** ([`super::online_game_session`]).
//! Direct P2P is great when it connects, but it fails silently when the two peers
//! can't establish a gossip link (NAT, or two instances on one machine during
//! `just dev2`). When that happens the opponent never sees your moves.
//!
//! This bridge adds a **second, reliable transport** over the same VPS relay that
//! already carries lobby handshakes (`/p2p/message` + `/p2p/poll`, proven by the
//! JOIN_ACK flow). It is a *parallel* path, not a replacement:
//!
//! * **Outgoing** — [`relay_send`] serialises a [`NetworkMessage`] and posts it to
//!   the relay mailbox for the current game.
//! * **Incoming** — [`relay_poll_incoming`] polls the mailbox and injects decoded
//!   messages as [`NetworkEvent::MessageReceived`] into the *same* channel gossip
//!   uses, so they run through the identical replay/nonce/roster/causal/apply
//!   pipeline in `handle_network_events`.
//!
//! # Safety of dual delivery
//! Running both transports is safe and self-deduplicating: every `Move`/`Resign`
//! carries a monotonic `nonce`, and `handle_network_events` rejects any message
//! whose nonce is below the expected value. So if a move arrives over *both* gossip
//! and the relay, the second copy is dropped — first-wins, no double-apply.
//!
//! # Relay routing
//! The backend relay stores messages in per-role queues keyed by `from_node_id`
//! (host → `host_messages`, joiner → `joiner_messages`) and each side polls the
//! other's queue. This requires host and joiner to have **distinct node ids**
//! (see [`super::identity`]).

use bevy::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use crate::multiplayer::network::online_game_session::OnlineGameSession;
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::types::{NetworkEvent, OnlineNetworkState};

/// Wire prefix distinguishing relayed `NetworkMessage`s from lobby control
/// messages (e.g. `JOIN_ACK:`) that share the same mailbox.
const NETMSG_PREFIX: &str = "NETMSG:";

/// How often to poll the relay for incoming in-game messages.
///
/// This only matters when direct Iroh gossip fails to link (NAT traversal
/// failure) and delivery falls back entirely to this poller — every move
/// then sits in the mailbox for up to one interval before being picked up.
/// Kept short since each poll is a cheap short-lived HTTP GET.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Base58 of the local Iroh node id — matches the id used to announce/join, which
/// is what the backend relay keys mailboxes on.
fn local_node_b58(network_state: &OnlineNetworkState) -> Option<String> {
    network_state
        .node_id
        .as_ref()
        .map(|id| bs58::encode(id.as_bytes()).into_string())
}

/// Serialise and post a `NetworkMessage` to the VPS relay for `game_id`.
/// Fire-and-forget (runs on a worker thread); failures are logged, not fatal —
/// gossip may still deliver it.
pub fn relay_send(game_id: &str, from_node_b58: &str, msg: &NetworkMessage) {
    let json = match serde_json::to_string(msg) {
        Ok(j) => j,
        Err(e) => {
            warn!("[relay-bridge] serialize failed: {e}");
            return;
        }
    };
    let payload = format!("{NETMSG_PREFIX}{json}");
    let game_id = game_id.to_string();
    let node = from_node_b58.to_string();
    std::thread::spawn(move || {
        if let Err(e) = crate::multiplayer::vps_client::p2p_send_message(game_id, &node, &payload) {
            debug!("[relay-bridge] relay send failed (gossip may cover it): {e}");
        }
    });
}

/// Per-frame bookkeeping for the incoming relay poller.
#[derive(Resource)]
pub struct RelayBridge {
    /// Index into the relay mailbox we've consumed up to.
    poll_index: usize,
    last_poll: Option<Instant>,
    /// True while a poll thread is in flight (avoids overlapping polls).
    inflight: Arc<AtomicBool>,
    /// Poll threads report the mailbox's next index back through this channel.
    result_tx: crossbeam_channel::Sender<usize>,
    result_rx: crossbeam_channel::Receiver<usize>,
}

impl Default for RelayBridge {
    fn default() -> Self {
        let (result_tx, result_rx) = crossbeam_channel::unbounded();
        Self {
            poll_index: 0,
            last_poll: None,
            inflight: Arc::new(AtomicBool::new(false)),
            result_tx,
            result_rx,
        }
    }
}

/// Poll the relay for in-game messages and inject them into the gossip pipeline.
///
/// Only runs while a online game session is active. Decoded messages are sent as
/// `NetworkEvent::MessageReceived`, identical to how gossip delivers them, so all
/// downstream validation/apply logic is shared.
pub fn relay_poll_incoming(
    mut bridge: ResMut<RelayBridge>,
    session: Res<OnlineGameSession>,
    network_state: Res<OnlineNetworkState>,
) {
    // Absorb results from any completed poll thread first.
    while let Ok(next_index) = bridge.result_rx.try_recv() {
        if next_index > bridge.poll_index {
            bridge.poll_index = next_index;
        }
    }

    if !session.is_configured() {
        return;
    }
    let (Some(node), Some(event_tx)) = (
        local_node_b58(&network_state),
        network_state.event_sender.clone(),
    ) else {
        return;
    };

    let due = bridge
        .last_poll
        .map(|t| t.elapsed() >= POLL_INTERVAL)
        .unwrap_or(true);
    if !due || bridge.inflight.load(Ordering::Relaxed) {
        return;
    }
    bridge.last_poll = Some(Instant::now());
    bridge.inflight.store(true, Ordering::Relaxed);

    let game_id = session.game_id.clone();
    let since = bridge.poll_index;
    let inflight = bridge.inflight.clone();
    let result_tx = bridge.result_tx.clone();

    std::thread::spawn(move || {
        match crate::multiplayer::vps_client::p2p_poll_messages(game_id, &node, since) {
            Ok((messages, next_index)) => {
                for raw in &messages {
                    let Some(json) = raw.strip_prefix(NETMSG_PREFIX) else {
                        continue;
                    };
                    match serde_json::from_str::<NetworkMessage>(json) {
                        Ok(msg) => {
                            // Feed the shared pipeline; nonce dedup handles gossip overlap.
                            let _ = event_tx.send(NetworkEvent::MessageReceived(msg));
                        }
                        Err(e) => debug!("[relay-bridge] decode failed: {e}"),
                    }
                }
                let _ = result_tx.send(next_index);
            }
            Err(e) => debug!("[relay-bridge] poll failed: {e}"),
        }
        inflight.store(false, Ordering::Relaxed);
    });
}

/// Registers the relay bridge resource + incoming poller.
pub struct RelayBridgePlugin;

impl Plugin for RelayBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RelayBridge>()
            .add_systems(Update, relay_poll_incoming);
    }
}
