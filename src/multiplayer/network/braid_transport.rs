use bevy::prelude::*;
use braid_chess::message::ChatPayload;
use braid_chess::{
    journal::Journal,
    subscriber::{ChessDelivery, DeliveryReceipt},
    ChessResource,
};
use braid_chess::{ChessMessage, ChessSubscriber, MovePayload};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::multiplayer::network::online_game_session::{OnlineChatMessage, OnlineGameSession};
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::types::{CausalChainState, NetworkEvent};
use crate::multiplayer::TokioRuntime;

const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

const GENESIS_PARENT: &str = "0";

mod publish;
use publish::publish;
pub(super) use publish::retry_pending;

pub(crate) fn open_journal(base_url: &str, game_id: &str) -> std::io::Result<Journal> {
    Journal::open(
        &super::device_id::data_dir().join("game-sync"),
        &format!(
            "{}/{}/{}",
            base_url.trim_end_matches('/'),
            game_id,
            super::device_id::device_id()
        ),
    )
}

pub struct BraidStreamHeads {
    moves: String,
    chat: String,
    self_published: std::collections::HashSet<String>,
}

impl Default for BraidStreamHeads {
    fn default() -> Self {
        Self {
            moves: GENESIS_PARENT.to_string(),
            chat: GENESIS_PARENT.to_string(),
            self_published: std::collections::HashSet::new(),
        }
    }
}

impl BraidStreamHeads {
    fn head(&self, stream: &str) -> String {
        if stream == "chat" {
            self.chat.clone()
        } else {
            self.moves.clone()
        }
    }

    fn set_head(&mut self, stream: &str, version: String) {
        if stream == "chat" {
            self.chat = version;
        } else {
            self.moves = version;
        }
    }
}

pub type SharedStreamHeads = std::sync::Arc<std::sync::Mutex<BraidStreamHeads>>;
type AppliedCursors = std::sync::Arc<
    std::sync::Mutex<std::collections::HashMap<&'static str, Vec<braid_http::Version>>>,
>;

fn version_of(message: &ChessMessage) -> Option<String> {
    Some(match message {
        ChessMessage::Move(p) => braid_chess::version_hash(&p.fen_after, p.move_number),
        ChessMessage::Resign { player } => {
            braid_chess::version_hash(&format!("resign:{player}"), 0)
        }
        ChessMessage::Chat(p) => {
            braid_chess::version_hash(&format!("chat:{}:{}", p.player, p.timestamp_ms), 0)
        }
        ChessMessage::SessionInfo { .. } => braid_chess::version_hash(
            &serde_json::to_string(message).expect("chess message serializes"),
            0,
        ),
        _ => return None,
    })
}

fn stream_of(message: &ChessMessage) -> &'static str {
    if matches!(message, ChessMessage::Chat(_)) {
        "chat"
    } else {
        "moves"
    }
}

pub fn publish_move(
    base_url: String,
    game_id: String,
    sender_identity: String,
    session_token: String,
    payload: MovePayload,
    content_version: String,
    heads: SharedStreamHeads,
) {
    publish(
        base_url,
        game_id,
        "moves",
        sender_identity,
        session_token,
        ChessMessage::Move(payload),
        content_version,
        heads,
    );
}

pub fn publish_resign(
    base_url: String,
    game_id: String,
    sender_identity: String,
    session_token: String,
    resigning_player: String,
    heads: SharedStreamHeads,
) {
    let content_version = braid_chess::version_hash(&format!("resign:{resigning_player}"), 0);
    publish(
        base_url,
        game_id,
        "moves",
        sender_identity,
        session_token,
        ChessMessage::Resign {
            player: resigning_player,
        },
        content_version,
        heads,
    );
}

pub fn publish_chat(
    base_url: String,
    game_id: String,
    sender_identity: String,
    session_token: String,
    player: String,
    text: String,
    timestamp_ms: u64,
    heads: SharedStreamHeads,
) {
    let content_version = braid_chess::version_hash(&format!("chat:{player}:{timestamp_ms}"), 0);
    publish(
        base_url,
        game_id,
        "chat",
        sender_identity,
        session_token,
        ChessMessage::Chat(ChatPayload {
            player,
            text,
            timestamp_ms,
        }),
        content_version,
        heads,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn publish_session_info(
    base_url: String,
    game_id: String,
    sender_identity: String,
    session_token: String,
    wallet_pubkey: String,
    session_pubkey: String,
    signing_pubkey: String,
    expires_at: i64,
    heads: SharedStreamHeads,
) {
    let message = ChessMessage::SessionInfo {
        player_pubkey: wallet_pubkey,
        session_pubkey,
        signing_pubkey,
        expires_at,
    };
    let content_version = version_of(&message).expect("session info has a version");
    publish(
        base_url,
        game_id,
        "moves",
        sender_identity,
        session_token,
        message,
        content_version,
        heads,
    );
}

#[derive(Resource, Default)]
pub struct BraidTransportState {
    heads: SharedStreamHeads,
    game_id: String,
    rx: Option<crossbeam_channel::Receiver<ChessMessage>>,
    deliveries: Option<tokio::sync::mpsc::Receiver<ChessDelivery>>,
    subscription_task: Option<tokio::task::JoinHandle<()>>,
    journal: Option<Journal>,
    pending: std::collections::HashMap<String, DeliveryReceipt>,
    cursors: AppliedCursors,
    pub connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for BraidTransportState {
    fn drop(&mut self) {
        if let Some(task) = self.subscription_task.take() {
            task.abort();
        }
    }
}

impl BraidTransportState {
    #[cfg(test)]
    pub(crate) fn new_for_test(
        game_id: String,
        rx: crossbeam_channel::Receiver<ChessMessage>,
    ) -> Self {
        Self {
            game_id,
            rx: Some(rx),
            connected: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            heads: SharedStreamHeads::default(),
            deliveries: None,
            subscription_task: None,
            journal: None,
            pending: Default::default(),
            cursors: Default::default(),
        }
    }

    pub fn heads(&self) -> SharedStreamHeads {
        self.heads.clone()
    }

    pub fn reset(&mut self) {
        self.game_id.clear();
        self.rx = None;
        self.deliveries = None;
        if let Some(task) = self.subscription_task.take() {
            task.abort();
        }
        self.journal = None;
        self.pending.clear();
        self.cursors = Default::default();
        self.connected
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.heads = SharedStreamHeads::default();
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn confirm(&self, receipt: DeliveryReceipt, stream: &'static str, fen: Option<String>) {
        let Some(journal) = &self.journal else {
            return;
        };
        match journal.applied(stream, receipt.versions().to_vec(), fen) {
            Ok(()) => {
                if let Ok(mut cursors) = self.cursors.lock() {
                    cursors.insert(stream, receipt.versions().to_vec());
                }
                let _ = receipt.acknowledge();
            }
            Err(error) => warn!("[braid] could not save applied checkpoint: {error}"),
        }
    }

    pub(crate) fn confirm_move(&mut self, version: &str, fen: &str) {
        if let Some(receipt) = self.pending.remove(version) {
            self.confirm(receipt, "moves", Some(fen.to_owned()));
        }
    }

    pub(crate) fn reject_move(&mut self, version: &str) {
        self.pending.remove(version);
    }
}

pub fn ensure_subscribed(
    state: &mut BraidTransportState,
    base_url: String,
    game_id: String,
    rt: &tokio::runtime::Handle,
) {
    if state.game_id == game_id && state.deliveries.is_some() {
        return;
    }
    state.reset();
    let journal = match open_journal(&base_url, &game_id) {
        Ok(journal) => journal,
        Err(error) => {
            warn!("[braid] could not open game journal: {error}");
            return;
        }
    };
    state.game_id = game_id.clone();
    state.journal = Some(journal);
    state
        .connected
        .store(false, std::sync::atomic::Ordering::Relaxed);

    let (tx, rx) = tokio::sync::mpsc::channel::<ChessDelivery>(2);
    state.deliveries = Some(rx);

    state.subscription_task = Some(spawn_reconnecting_subscription(
        base_url,
        game_id,
        tx,
        state.connected.clone(),
        state.cursors.clone(),
        rt,
    ));
}

fn spawn_reconnecting_subscription(
    base_url: String,
    game_id: String,
    tx: tokio::sync::mpsc::Sender<ChessDelivery>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
    cursors: AppliedCursors,
    rt: &tokio::runtime::Handle,
) -> tokio::task::JoinHandle<()> {
    rt.spawn(async move {
        let mut backoff = MIN_BACKOFF;
        // Show reconnection notices only after a prior successful connection was lost.
        let mut recovering = false;
        loop {
            if tx.is_closed() { return; }
            let sub = match ChessSubscriber::new(&base_url, &game_id) {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        "[NET] Move sync backup (Braid) couldn't start for game {game_id}: {e}"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                    recovering = true;
                    continue;
                }
            };

            // Resume both streams from their last durable application acknowledgment.
            let (moves_parents, chat_parents) = match cursors.lock() {
                Ok(cursors) => (cursors.get("moves").cloned().unwrap_or_default(), cursors.get("chat").cloned().unwrap_or_default()),
                Err(_) => return,
            };
            let moves = sub.subscribe_acknowledged(ChessResource::moves(&game_id), moves_parents).await;
            let chat = sub.subscribe_acknowledged(ChessResource::chat(&game_id), chat_parents).await;
            let (moves_rx, chat_rx) = match (moves, chat) {
                (Ok((m, _)), Ok((c, _))) => (m, c),
                (Err(e), _) | (_, Err(e)) => {
                    if matches!(e, braid_chess::BraidChessError::HttpStatus(410)) {
                        // Replay through the engine and deduplication layer after cursor expiry.
                        if let Ok(mut cursors) = cursors.lock() { cursors.clear(); }
                    }
                    warn!(
                        "[NET] Move sync backup (Braid) couldn't reach the server for game {game_id}: {e} — retrying"
                    );
                    connected.store(false, std::sync::atomic::Ordering::Relaxed);
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                    recovering = true;
                    continue;
                }
            };
            if recovering {
                info!("[NET] Move sync backup (Braid) reconnected for game {game_id}");
            } else {
                debug!("[braid-transport] subscribed to moves+chat for game {game_id}");
            }
            backoff = MIN_BACKOFF; // reset after a successful (re)connect
            connected.store(true, std::sync::atomic::Ordering::Relaxed);

            loop {
                tokio::select! {
                    msg = moves_rx.recv() => {
                        match msg {
                            Ok(m) => { if tx.send(m).await.is_err() { return; } }
                            Err(_) => break,
                        }
                    }
                    msg = chat_rx.recv() => {
                        match msg {
                            Ok(m) => { if tx.send(m).await.is_err() { return; } }
                            Err(_) => break,
                        }
                    }
                }
            }
            connected.store(false, std::sync::atomic::Ordering::Relaxed);
            warn!(
                "[NET] Move sync backup (Braid) lost connection for game {game_id} — reconnecting"
            );
            recovering = true;
            tokio::time::sleep(MIN_BACKOFF).await;
        }
    })
}

pub fn drain_braid_messages(
    mut state: ResMut<BraidTransportState>,
    session: Res<OnlineGameSession>,
    mut causal: ResMut<CausalChainState>,
    mut network_events: MessageWriter<NetworkEvent>,
    mut resign_events: MessageWriter<crate::game::events::ResignEvent>,
    mut chat_events: MessageWriter<OnlineChatMessage>,
) {
    if state.rx.is_none() && state.deliveries.is_none() {
        return;
    }
    let game_id_u64 =
        crate::multiplayer::network::online_game_session::numeric_game_id(&session.game_id);

    loop {
        let (msg, receipt) = if let Some(rx) = state.deliveries.as_mut() {
            let Ok(delivery) = rx.try_recv() else {
                break;
            };
            (delivery.message, Some(delivery.receipt))
        } else {
            let Some(rx) = &state.rx else {
                break;
            };
            let Ok(msg) = rx.try_recv() else {
                break;
            };
            (msg, None)
        };
        let stream = stream_of(&msg);
        let fen = if let ChessMessage::Move(payload) = &msg {
            Some(payload.fen_after.clone())
        } else {
            None
        };
        // Advance to the accepted stream head and discard self-published echoes
        // so moves and chat are not applied twice.
        let mut is_self_echo = false;
        if let Some(version) = version_of(&msg) {
            if let Ok(mut h) = state.heads.lock() {
                h.set_head(stream_of(&msg), version.clone());
                is_self_echo = h.self_published.remove(&version);
            }
        }
        if is_self_echo {
            if let Some(receipt) = receipt {
                state.confirm(receipt, stream, fen);
            }
            continue;
        }

        match msg {
            ChessMessage::Move(payload) => {
                let version = braid_chess::version_hash(&payload.fen_after, payload.move_number);
                let already_applied = causal
                    .applied_versions
                    .get(&game_id_u64)
                    .is_some_and(|versions| versions.contains(&version));
                if already_applied {
                    if let Some(receipt) = receipt {
                        state.confirm(receipt, "moves", Some(payload.fen_after));
                    }
                    continue;
                }
                if let Some(receipt) = receipt {
                    state.pending.insert(version.clone(), receipt);
                }
                let already_pending = causal
                    .pending_versions
                    .get(&game_id_u64)
                    .is_some_and(|versions| versions.contains(&version));
                if already_pending {
                    continue;
                }
                causal
                    .pending_versions
                    .entry(game_id_u64)
                    .or_default()
                    .insert(version.clone());
                network_events.write(NetworkEvent::BraidMove(NetworkMessage::Move {
                    game_id: game_id_u64,
                    turn: payload.move_number as u16,
                    move_uci: payload.uci,
                    next_fen: payload.fen_after,
                    nonce: 0,
                    timestamp_ms: 0,
                    signer_pubkey: Vec::new(),
                    seq: 0,
                    parent_version: String::new(),
                }));
                continue;
            }
            ChessMessage::Resign { player } => {
                let winner = if player == "white" { "black" } else { "white" };
                resign_events.write(crate::game::events::ResignEvent {
                    winner: winner.to_string(),
                    remote: true,
                });
            }
            ChessMessage::Chat(payload) => {
                chat_events.write(OnlineChatMessage {
                    player: payload.player,
                    text: payload.text,
                    timestamp_ms: payload.timestamp_ms,
                });
            }
            ChessMessage::SessionInfo {
                player_pubkey,
                session_pubkey,
                signing_pubkey,
                expires_at,
            } => {
                let claim_trusted = match causal.verified_wallets.get(&game_id_u64) {
                    Some((white, black)) => {
                        let claimed = player_pubkey.clone();
                        claimed == *white || claimed == *black
                    }
                    None => true,
                };

                if !claim_trusted {
                    warn!(
                        "[braid-transport] Ignored spoofed SessionInfo for game {}: claimed player_pubkey {} is not in verified wallet pair {:?}",
                        game_id_u64,
                        player_pubkey,
                        causal.verified_wallets.get(&game_id_u64)
                    );
                    continue;
                }

                // Build the same signer roster as gossip; Braid supplies dropped SessionInfo messages.
                let Ok(key) = bs58::decode(&signing_pubkey).into_vec() else {
                    warn!("[braid-transport] SessionInfo had an unparseable signing_pubkey");
                    continue;
                };
                let entry = causal.roster.entry(game_id_u64).or_default();
                if !entry.contains(&key) && entry.len() < 2 {
                    entry.push(key);
                    info!(
                        "[braid-transport] Roster for game {} now has {} entry(ies) via Braid",
                        game_id_u64,
                        entry.len()
                    );
                }

                // Re-emit SessionInfo so the handler sets opponent_pubkey for finalization.
                // Updating the signer roster alone does not update SolanaIntegrationState.
                #[cfg(feature = "solana")]
                {
                    use solana_sdk::pubkey::Pubkey;
                    use std::str::FromStr;
                    match (
                        Pubkey::from_str(&player_pubkey),
                        Pubkey::from_str(&session_pubkey),
                        Pubkey::from_str(&signing_pubkey),
                    ) {
                        (Ok(player_pubkey), Ok(session_pubkey), Ok(signing_pubkey)) => {
                            network_events.write(NetworkEvent::MessageReceived(
                                NetworkMessage::SessionInfo {
                                    game_id: game_id_u64,
                                    player_pubkey,
                                    session_pubkey,
                                    signing_pubkey,
                                    expires_at,
                                },
                            ));
                        }
                        _ => {
                            warn!(
                                "[braid-transport] SessionInfo for game {} had an unparseable pubkey — opponent_pubkey not updated via Braid fallback",
                                game_id_u64
                            );
                        }
                    }
                }
                // Without Solana there is no opponent key to record.
                #[cfg(not(feature = "solana"))]
                let _ = (session_pubkey, expires_at);
            }
            // Draw offers use gossip. Clock and EngineAnalysis use separate Braid streams.
            _ => {}
        }
        if let Some(receipt) = receipt {
            state.confirm(receipt, stream, fen);
        }
    }
}

fn sync_braid_subscription(
    mut state: ResMut<BraidTransportState>,
    session: Res<OnlineGameSession>,
    tokio_runtime: Res<TokioRuntime>,
) {
    if !session.is_configured() {
        return;
    }
    ensure_subscribed(
        &mut state,
        session.base_url.clone(),
        session.game_id.clone(),
        tokio_runtime.0.handle(),
    );
}

fn acknowledge_applied_moves(
    mut state: ResMut<BraidTransportState>,
    mut results: MessageReader<crate::game::events::NetworkMoveResult>,
) {
    for result in results.read() {
        if let Some(version) = &result.version {
            match &result.fen {
                Some(fen) => state.confirm_move(version, fen),
                None => state.reject_move(version),
            }
        }
    }
}

pub struct BraidTransportPlugin;

impl Plugin for BraidTransportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BraidTransportState>()
            .add_message::<crate::game::events::NetworkMoveResult>()
            .add_systems(
                Update,
                (sync_braid_subscription, drain_braid_messages)
                    .chain()
                    .in_set(crate::core::schedule::ReceiveSet::Transport),
            )
            .add_systems(
                Update,
                acknowledge_applied_moves.in_set(crate::core::schedule::SendSet::Ack),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_chess::message::{ChatPayload, MovePayload};

    fn move_msg(fen_after: &str, move_number: u32) -> ChessMessage {
        ChessMessage::Move(MovePayload::from_uci("e2e4", fen_after, move_number, "p"))
    }

    #[test]
    fn head_advances_on_opponent_moves_not_just_our_own() {
        let mut heads = BraidStreamHeads::default();
        assert_eq!(heads.head("moves"), GENESIS_PARENT);

        // Opponent's move arrives over the subscription.
        let opponent = move_msg("fen-opponent", 1);
        let v_opponent = version_of(&opponent).unwrap();
        heads.set_head(stream_of(&opponent), v_opponent.clone());

        // Our next publish must chain off it. Before the fix this still read
        // "0" (or our own last version), which the backend rejected with 409.
        assert_eq!(heads.head("moves"), v_opponent);
    }

    #[test]
    fn move_and_chat_heads_are_independent() {
        let mut heads = BraidStreamHeads::default();

        let mv = move_msg("fen1", 1);
        heads.set_head(stream_of(&mv), version_of(&mv).unwrap());
        let moves_head = heads.head("moves");

        let chat = ChessMessage::Chat(ChatPayload {
            player: "alice".to_string(),
            text: "gg".to_string(),
            timestamp_ms: 7,
        });
        assert_eq!(stream_of(&chat), "chat");
        heads.set_head(stream_of(&chat), version_of(&chat).unwrap());

        assert_eq!(
            heads.head("moves"),
            moves_head,
            "chat must not move the moves head"
        );
        assert_ne!(heads.head("chat"), GENESIS_PARENT);
        assert_ne!(heads.head("chat"), moves_head);
    }

    #[test]
    fn version_of_matches_the_publish_side_hashes() {
        let mv = move_msg("fen-after", 3);
        assert_eq!(
            version_of(&mv).unwrap(),
            braid_chess::version_hash("fen-after", 3)
        );

        let resign = ChessMessage::Resign {
            player: "white".to_string(),
        };
        assert_eq!(
            version_of(&resign).unwrap(),
            braid_chess::version_hash("resign:white", 0)
        );

        let chat = ChessMessage::Chat(ChatPayload {
            player: "bob".to_string(),
            text: "hi".to_string(),
            timestamp_ms: 42,
        });
        assert_eq!(
            version_of(&chat).unwrap(),
            braid_chess::version_hash("chat:bob:42", 0)
        );

        let session = ChessMessage::SessionInfo {
            player_pubkey: "wallet1".to_string(),
            session_pubkey: "s".to_string(),
            signing_pubkey: "g".to_string(),
            expires_at: 0,
        };
        assert_eq!(
            version_of(&session).unwrap(),
            braid_chess::version_hash(&serde_json::to_string(&session).unwrap(), 0)
        );
        let renewed = ChessMessage::SessionInfo {
            player_pubkey: "wallet1".into(),
            session_pubkey: "renewed".into(),
            signing_pubkey: "g".into(),
            expires_at: 1,
        };
        assert_ne!(version_of(&session), version_of(&renewed));
    }

    #[test]
    fn own_publishes_are_recognised_as_echoes_exactly_once() {
        let mut heads = BraidStreamHeads::default();
        let mine = move_msg("fen-mine", 1);
        let v = version_of(&mine).unwrap();

        // publish() claims the version up-front.
        heads.self_published.insert(v.clone());

        // First delivery is our own echo.
        assert!(heads.self_published.remove(&v), "own echo must be detected");
        // A second, genuinely-remote message with the same version would not
        // be swallowed (the claim is consumed, not permanent).
        assert!(!heads.self_published.remove(&v));
    }

    #[test]
    fn opponent_session_info_is_not_treated_as_an_echo() {
        let mut heads = BraidStreamHeads::default();
        let ours = ChessMessage::SessionInfo {
            player_pubkey: "us".to_string(),
            session_pubkey: "s".to_string(),
            signing_pubkey: "g".to_string(),
            expires_at: 0,
        };
        heads.self_published.insert(version_of(&ours).unwrap());

        let theirs = ChessMessage::SessionInfo {
            player_pubkey: "them".to_string(),
            session_pubkey: "s2".to_string(),
            signing_pubkey: "g2".to_string(),
            expires_at: 0,
        };
        let v_theirs = version_of(&theirs).unwrap();
        assert!(
            !heads.self_published.remove(&v_theirs),
            "opponent SessionInfo must be delivered, not swallowed as an echo"
        );
    }
}
