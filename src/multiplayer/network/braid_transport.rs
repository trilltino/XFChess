use bevy::prelude::*;
use braid_chess::message::ChatPayload;
use braid_chess::{
    journal::Journal, subscriber::DeliveryReceipt, ChessOutbox, ChessResource, OutboxConfig,
    OutboxHooks, StreamCursor, StreamHandle, SupervisorConfig,
};
use braid_chess::{ChessMessage, MovePayload};
use tracing::{info, warn};

use crate::multiplayer::network::online_game_session::{OnlineChatMessage, OnlineGameSession};
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::types::{CausalChainState, NetworkEvent};
use crate::multiplayer::TokioRuntime;

const GENESIS_PARENT: &str = "0";

mod draws;
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
    /// Durable publisher for this game; absent until the subscription is set up.
    outbox: Option<std::sync::Arc<ChessOutbox>>,
}

impl Default for BraidStreamHeads {
    fn default() -> Self {
        Self {
            moves: GENESIS_PARENT.to_string(),
            chat: GENESIS_PARENT.to_string(),
            self_published: std::collections::HashSet::new(),
            outbox: None,
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

fn version_of(message: &ChessMessage) -> Option<String> {
    Some(match message {
        ChessMessage::Move(p) => braid_chess::version_hash(&p.fen_after, p.move_number),
        ChessMessage::Resign { player } => {
            braid_chess::version_hash(&format!("resign:{player}"), 0)
        }
        ChessMessage::Chat(p) => {
            braid_chess::version_hash(&format!("chat:{}:{}", p.player, p.timestamp_ms), 0)
        }
        ChessMessage::SessionInfo { .. } => {
            braid_chess::version_hash(&serde_json::to_string(message).ok()?, 0)
        }
        ChessMessage::OfferDraw { .. }
        | ChessMessage::AcceptDraw { .. }
        | ChessMessage::DeclineDraw { .. } => braid_chess::version_hash(&message.draw_key()?, 0),
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

/// Publish a draw offer, acceptance or refusal on the moves stream, where it is
/// ordered with the moves it refers to and survives either player's disconnect.
pub fn publish_draw(
    base_url: String,
    game_id: String,
    sender_identity: String,
    session_token: String,
    message: ChessMessage,
    heads: SharedStreamHeads,
) {
    let Some(content_version) = version_of(&message) else {
        warn!("[braid] publish_draw called with a message that is not a draw action");
        return;
    };
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
    let Some(content_version) = version_of(&message) else {
        warn!("[braid] could not serialize session info");
        return;
    };
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
    /// Moves and chat are supervised independently; dropping a handle cancels its task.
    moves: Option<StreamHandle>,
    chat: Option<StreamHandle>,
    journal: Option<Journal>,
    pending: std::collections::HashMap<String, DeliveryReceipt>,
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
            heads: SharedStreamHeads::default(),
            moves: None,
            chat: None,
            journal: None,
            pending: Default::default(),
        }
    }

    pub fn heads(&self) -> SharedStreamHeads {
        self.heads.clone()
    }

    pub fn reset(&mut self) {
        self.game_id.clear();
        self.rx = None;
        self.moves = None;
        self.chat = None;
        self.journal = None;
        self.pending.clear();
        self.heads = SharedStreamHeads::default();
    }

    /// The move stream is the one the game depends on; chat health is reported separately.
    pub fn is_connected(&self) -> bool {
        self.moves.as_ref().is_some_and(StreamHandle::is_online)
    }

    pub fn chat_is_connected(&self) -> bool {
        self.chat.as_ref().is_some_and(StreamHandle::is_online)
    }

    fn handle(&self, stream: &str) -> Option<&StreamHandle> {
        if stream == "chat" {
            self.chat.as_ref()
        } else {
            self.moves.as_ref()
        }
    }

    /// Moves are drained first so a busy chat stream can never delay a move.
    fn next_delivery(&mut self) -> Option<(ChessMessage, Option<DeliveryReceipt>)> {
        let delivery = self
            .moves
            .as_mut()
            .and_then(StreamHandle::try_recv)
            .or_else(|| self.chat.as_mut().and_then(StreamHandle::try_recv));
        if let Some(delivery) = delivery {
            return Some((delivery.message, Some(delivery.receipt)));
        }
        self.rx
            .as_ref()?
            .try_recv()
            .ok()
            .map(|message| (message, None))
    }

    fn confirm(&self, receipt: DeliveryReceipt, stream: &'static str, fen: Option<String>) {
        let Some(journal) = &self.journal else {
            return;
        };
        match journal.applied(stream, receipt.versions().to_vec(), fen) {
            Ok(()) => {
                if let Some(handle) = self.handle(stream) {
                    handle.cursor().set(receipt.versions().to_vec());
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

/// Create the durable publisher that sends this game's events in order, survives restarts,
/// and adopts anything the earlier journal still holds.
fn install_outbox(
    state: &BraidTransportState,
    base_url: &str,
    game_id: &str,
    journal: Journal,
    rt: &tokio::runtime::Handle,
) {
    let accepted_heads = std::sync::Arc::downgrade(&state.heads);
    let seat_game = game_id.to_owned();
    let hooks = OutboxHooks {
        headers: std::sync::Arc::new(crate::multiplayer::network::vps::auth_headers),
        is_seat_superseded: std::sync::Arc::new(super::device_id::is_seat_superseded_body),
        on_seat_superseded: std::sync::Arc::new(move || {
            if let Ok(id) = seat_game.parse() {
                super::device_id::mark_seat_superseded(id);
            }
        }),
        on_accepted: std::sync::Arc::new(move |stream, version| {
            if let Some(heads) = accepted_heads.upgrade() {
                if let Ok(mut heads) = heads.lock() {
                    heads.set_head(stream.as_str(), version.to_owned());
                }
            }
        }),
    };
    let config = OutboxConfig {
        base_url: base_url.to_owned(),
        game_id: game_id.to_owned(),
        device_id: super::device_id::device_id().to_owned(),
        directory: journal.directory().to_path_buf(),
        put_timeout: std::time::Duration::from_secs(20),
        max_pending: 64,
    };
    match ChessOutbox::new(config, hooks, rt.clone(), Some(journal)) {
        Ok(outbox) => {
            if let Ok(mut heads) = state.heads.lock() {
                heads.outbox = Some(std::sync::Arc::new(outbox));
            }
        }
        Err(error) => warn!("[braid] could not start the event outbox: {error}"),
    }
}

pub fn ensure_subscribed(
    state: &mut BraidTransportState,
    base_url: String,
    game_id: String,
    rt: &tokio::runtime::Handle,
) {
    if state.game_id == game_id && state.moves.is_some() {
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
    state.journal = Some(journal.clone());
    install_outbox(state, &base_url, &game_id, journal, rt);
    let spawn = |resource: ChessResource| {
        StreamHandle::spawn(
            base_url.clone(),
            game_id.clone(),
            resource,
            StreamCursor::default(),
            SupervisorConfig::default(),
            rt,
        )
    };
    state.moves = Some(spawn(ChessResource::moves(&game_id)));
    state.chat = Some(spawn(ChessResource::chat(&game_id)));
}

/// Reconnect both streams now, resuming from what was already acknowledged, instead of
/// waiting out the supervisor's backoff (up to 30 s). Does nothing before the first
/// subscription.
pub fn restart_streams(
    state: &mut BraidTransportState,
    base_url: &str,
    game_id: &str,
    rt: &tokio::runtime::Handle,
) {
    if state.game_id != game_id || state.moves.is_none() {
        return;
    }
    let respawn = |old: Option<StreamHandle>, resource: ChessResource| {
        let cursor = old
            .as_ref()
            .map(|handle| handle.cursor().clone())
            .unwrap_or_default();
        drop(old); // cancels the old task before the new one starts
        StreamHandle::spawn(
            base_url.to_owned(),
            game_id.to_owned(),
            resource,
            cursor,
            SupervisorConfig::default(),
            rt,
        )
    };
    state.moves = Some(respawn(state.moves.take(), ChessResource::moves(game_id)));
    state.chat = Some(respawn(state.chat.take(), ChessResource::chat(game_id)));
}

fn handle_refresh_game_state(
    mut refresh: MessageReader<crate::multiplayer::network::online_game_session::RefreshGameState>,
    mut state: ResMut<BraidTransportState>,
    session: Res<OnlineGameSession>,
    tokio_runtime: Res<TokioRuntime>,
    p2p: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    network_state: Option<Res<crate::multiplayer::OnlineNetworkState>>,
) {
    if refresh.read().count() == 0 || !session.is_configured() {
        return;
    }
    info!("[braid] refresh requested for game {}", session.game_id);
    restart_streams(
        &mut state,
        &session.base_url,
        &session.game_id,
        tokio_runtime.0.handle(),
    );
    // The gossip mesh may have lost the opponent too.
    if let (Some(peer), Some(network_state)) = (
        p2p.and_then(|conn| conn.peer_node_id.clone()),
        network_state,
    ) {
        crate::multiplayer::network::p2p::bootstrap_known_peer(&network_state, &peer);
    }
}

pub fn drain_braid_messages(
    mut state: ResMut<BraidTransportState>,
    session: Res<OnlineGameSession>,
    mut causal: ResMut<CausalChainState>,
    mut network_events: MessageWriter<NetworkEvent>,
    mut resign_events: MessageWriter<crate::game::events::ResignEvent>,
    mut chat_events: MessageWriter<OnlineChatMessage>,
    mut draw_offers: MessageWriter<crate::game::events::DrawOfferEvent>,
    mut draw_responses: MessageWriter<crate::game::events::DrawResponseEvent>,
    p2p: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    if state.rx.is_none() && state.moves.is_none() {
        return;
    }
    let game_id_u64 =
        crate::multiplayer::network::online_game_session::numeric_game_id(&session.game_id);

    for _ in 0..256 {
        let Some((msg, receipt)) = state.next_delivery() else {
            break;
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
            draw @ (ChessMessage::OfferDraw { .. }
            | ChessMessage::AcceptDraw { .. }
            | ChessMessage::DeclineDraw { .. }) => {
                // Gossip delivers draws live; this is the copy that survives a disconnect.
                let local_player =
                    p2p.as_ref()
                        .and_then(|c| c.player_color)
                        .map(|color| match color {
                            crate::rendering::pieces::PieceColor::White => "white",
                            crate::rendering::pieces::PieceColor::Black => "black",
                        });
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_millis() as u64);
                match draws::remote_draw(&draw, local_player, now_ms) {
                    Some(draws::RemoteDraw::Offer { player, ply }) => {
                        draw_offers.write(crate::game::events::DrawOfferEvent {
                            player,
                            remote: true,
                            ply: Some(ply),
                        });
                    }
                    Some(draws::RemoteDraw::Accept { player, ply }) => {
                        draw_responses.write(crate::game::events::DrawResponseEvent {
                            player,
                            accepted: true,
                            remote: true,
                            ply: Some(ply),
                        });
                    }
                    Some(draws::RemoteDraw::Decline { player, ply }) => {
                        draw_responses.write(crate::game::events::DrawResponseEvent {
                            player,
                            accepted: false,
                            remote: true,
                            ply: Some(ply),
                        });
                    }
                    None => {}
                }
            }
            // Clock and EngineAnalysis use separate Braid streams.
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
                (
                    handle_refresh_game_state,
                    sync_braid_subscription,
                    drain_braid_messages,
                )
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
    fn head_advances_on_opponent_moves_not_just_our_own() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut heads = BraidStreamHeads::default();
        assert_eq!(heads.head("moves"), GENESIS_PARENT);

        // Opponent's move arrives over the subscription.
        let opponent = move_msg("fen-opponent", 1);
        let v_opponent = version_of(&opponent).ok_or("missing test value")?;
        heads.set_head(stream_of(&opponent), v_opponent.clone());

        // Our next publish must chain off it. Before the fix this still read
        // "0" (or our own last version), which the backend rejected with 409.
        assert_eq!(heads.head("moves"), v_opponent);

        Ok(())
    }

    #[test]
    fn move_and_chat_heads_are_independent() -> Result<(), Box<dyn std::error::Error>> {
        let mut heads = BraidStreamHeads::default();

        let mv = move_msg("fen1", 1);
        heads.set_head(stream_of(&mv), version_of(&mv).ok_or("missing test value")?);
        let moves_head = heads.head("moves");

        let chat = ChessMessage::Chat(ChatPayload {
            player: "alice".to_string(),
            text: "gg".to_string(),
            timestamp_ms: 7,
        });
        assert_eq!(stream_of(&chat), "chat");
        heads.set_head(
            stream_of(&chat),
            version_of(&chat).ok_or("missing test value")?,
        );

        assert_eq!(
            heads.head("moves"),
            moves_head,
            "chat must not move the moves head"
        );
        assert_ne!(heads.head("chat"), GENESIS_PARENT);
        assert_ne!(heads.head("chat"), moves_head);

        Ok(())
    }

    #[test]
    fn version_of_matches_the_publish_side_hashes() -> Result<(), Box<dyn std::error::Error>> {
        let mv = move_msg("fen-after", 3);
        assert_eq!(
            version_of(&mv).ok_or("missing test value")?,
            braid_chess::version_hash("fen-after", 3)
        );

        let resign = ChessMessage::Resign {
            player: "white".to_string(),
        };
        assert_eq!(
            version_of(&resign).ok_or("missing test value")?,
            braid_chess::version_hash("resign:white", 0)
        );

        let chat = ChessMessage::Chat(ChatPayload {
            player: "bob".to_string(),
            text: "hi".to_string(),
            timestamp_ms: 42,
        });
        assert_eq!(
            version_of(&chat).ok_or("missing test value")?,
            braid_chess::version_hash("chat:bob:42", 0)
        );

        let session = ChessMessage::SessionInfo {
            player_pubkey: "wallet1".to_string(),
            session_pubkey: "s".to_string(),
            signing_pubkey: "g".to_string(),
            expires_at: 0,
        };
        assert_eq!(
            version_of(&session).ok_or("missing test value")?,
            braid_chess::version_hash(&serde_json::to_string(&session)?, 0)
        );
        let renewed = ChessMessage::SessionInfo {
            player_pubkey: "wallet1".into(),
            session_pubkey: "renewed".into(),
            signing_pubkey: "g".into(),
            expires_at: 1,
        };
        assert_ne!(version_of(&session), version_of(&renewed));

        Ok(())
    }

    #[test]
    fn own_publishes_are_recognised_as_echoes_exactly_once(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut heads = BraidStreamHeads::default();
        let mine = move_msg("fen-mine", 1);
        let v = version_of(&mine).ok_or("missing test value")?;

        // publish() claims the version up-front.
        heads.self_published.insert(v.clone());

        // First delivery is our own echo.
        assert!(heads.self_published.remove(&v), "own echo must be detected");
        // A second, genuinely-remote message with the same version would not
        // be swallowed (the claim is consumed, not permanent).
        assert!(!heads.self_published.remove(&v));

        Ok(())
    }

    #[test]
    fn opponent_session_info_is_not_treated_as_an_echo() -> Result<(), Box<dyn std::error::Error>> {
        let mut heads = BraidStreamHeads::default();
        let ours = ChessMessage::SessionInfo {
            player_pubkey: "us".to_string(),
            session_pubkey: "s".to_string(),
            signing_pubkey: "g".to_string(),
            expires_at: 0,
        };
        heads
            .self_published
            .insert(version_of(&ours).ok_or("missing test value")?);

        let theirs = ChessMessage::SessionInfo {
            player_pubkey: "them".to_string(),
            session_pubkey: "s2".to_string(),
            signing_pubkey: "g2".to_string(),
            expires_at: 0,
        };
        let v_theirs = version_of(&theirs).ok_or("missing test value")?;
        assert!(
            !heads.self_published.remove(&v_theirs),
            "opponent SessionInfo must be delivered, not swallowed as an echo"
        );

        Ok(())
    }
}
