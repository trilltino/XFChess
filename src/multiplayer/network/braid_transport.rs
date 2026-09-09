use bevy::prelude::*;
use braid_chess::message::ChatPayload;
use braid_chess::{ChessMessage, ChessSubscriber, MovePayload};
use serde::Serialize;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::multiplayer::network::online_game_session::{OnlineChatMessage, OnlineGameSession};
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::types::{CausalChainState, NetworkEvent};
use crate::multiplayer::TokioRuntime;

const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

const GENESIS_PARENT: &str = "0";

const MAX_PARENT_RETRIES: usize = 3;

// ── Stream head tracking ──────────────────────────────────────────────────

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

fn version_of(message: &ChessMessage) -> Option<String> {
    Some(match message {
        ChessMessage::Move(p) => braid_chess::version_hash(&p.fen_after, p.move_number),
        ChessMessage::Resign { player } => {
            braid_chess::version_hash(&format!("resign:{player}"), 0)
        }
        ChessMessage::Chat(p) => {
            braid_chess::version_hash(&format!("chat:{}:{}", p.player, p.timestamp_ms), 0)
        }
        ChessMessage::SessionInfo { player_pubkey, .. } => {
            braid_chess::version_hash(&format!("session:{player_pubkey}"), 0)
        }
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

// ── Publish (PUT) ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GameEventReq<'a> {
    #[serde(rename = "player_pubkey")]
    sender_identity: &'a str,
    session_token: &'a str,
    message: &'a ChessMessage,
    content_version: &'a str,
    content_parent: &'a str,
}

#[derive(serde::Deserialize)]
struct ParentMismatchResp {
    expected_parent: String,
}

fn publish(
    base_url: String,
    game_id: String,
    stream: &'static str,
    sender_identity: String,
    session_token: String,
    message: ChessMessage,
    content_version: String,
    heads: SharedStreamHeads,
) {
    std::thread::spawn(move || {
        let client = match crate::multiplayer::network::vps::client() {
            Ok(c) => c,
            Err(e) => {
                warn!("[braid-transport] client build failed: {e}");
                return;
            }
        };
        let url = format!(
            "{}/game/{}/{}",
            base_url.trim_end_matches('/'),
            game_id,
            stream
        );

        // Claim the version up-front so the echo of our own event is
        // recognised even if it arrives before this thread finishes.
        let mut parent = match heads.lock() {
            Ok(mut h) => {
                h.self_published.insert(content_version.clone());
                h.head(stream)
            }
            Err(_) => GENESIS_PARENT.to_string(),
        };

        for attempt in 0..=MAX_PARENT_RETRIES {
            let body = GameEventReq {
                sender_identity: &sender_identity,
                session_token: &session_token,
                message: &message,
                content_version: &content_version,
                content_parent: &parent,
            };
            match client.put(&url).json(&body).send() {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(mut h) = heads.lock() {
                        h.set_head(stream, content_version.clone());
                    }
                    debug!("[braid-transport] published to {game_id}/{stream}");
                    return;
                }
                Ok(resp) if resp.status() == reqwest::StatusCode::CONFLICT => {
                    match resp.json::<ParentMismatchResp>() {
                        Ok(m) => {
                            debug!(
                                "[braid-transport] re-chaining {stream} publish for game {game_id} onto head {} (attempt {})",
                                m.expected_parent,
                                attempt + 1
                            );
                            if let Ok(mut h) = heads.lock() {
                                h.set_head(stream, m.expected_parent.clone());
                            }
                            parent = m.expected_parent;
                        }
                        Err(e) => {
                            warn!(
                                "[NET] Move sync backup (Braid) got an unreadable conflict for {stream} on game {game_id}: {e}"
                            );
                            return;
                        }
                    }
                }
                Ok(resp) => {
                    let status = resp.status();
                    // Capture the backend's error body — it names the exact
                    // rejection reason (e.g. `NotAParticipant`), which is
                    // what made the v0.2.7 casual-P2P 403s diagnosable.
                    let body = resp.text().unwrap_or_default();
                    if status == reqwest::StatusCode::FORBIDDEN
                        || status == reqwest::StatusCode::UNAUTHORIZED
                    {
                        warn!(
                            "[braid-transport] Backend rejected our identity for {stream} on game {game_id} (HTTP {status}, sender={sender_identity}, body={body:?}) — check wallet-pubkey vs node-id selection for this game type"
                        );
                    } else {
                        warn!(
                            "[braid-transport] couldn't save {stream} for game {game_id}: server returned HTTP {status} (body={body:?})"
                        );
                    }
                    return;
                }
                Err(e) => {
                    warn!(
                        "[NET] Move sync backup (Braid) couldn't save {stream} for game {game_id}: {e}"
                    );
                    return;
                }
            }
        }
        warn!(
            "[NET] Move sync backup (Braid) gave up publishing {stream} for game {game_id} after {MAX_PARENT_RETRIES} re-chain attempts"
        );
    });
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
    let content_version = braid_chess::version_hash(&format!("session:{wallet_pubkey}"), 0);
    publish(
        base_url,
        game_id,
        "moves",
        sender_identity,
        session_token,
        ChessMessage::SessionInfo {
            player_pubkey: wallet_pubkey,
            session_pubkey,
            signing_pubkey,
            expires_at,
        },
        content_version,
        heads,
    );
}

// ── Subscribe (with reconnect) ─────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct BraidTransportState {
    heads: SharedStreamHeads,
    game_id: String,
    rx: Option<crossbeam_channel::Receiver<ChessMessage>>,
    pub connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
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
        }
    }

    pub fn heads(&self) -> SharedStreamHeads {
        self.heads.clone()
    }

    pub fn reset(&mut self) {
        self.game_id.clear();
        self.rx = None;
        self.connected
            .store(false, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut h) = self.heads.lock() {
            *h = BraidStreamHeads::default();
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }
}

pub fn ensure_subscribed(
    state: &mut BraidTransportState,
    base_url: String,
    game_id: String,
    rt: &tokio::runtime::Handle,
) {
    if state.game_id == game_id && state.rx.is_some() {
        return;
    }
    state.game_id = game_id.clone();
    state
        .connected
        .store(false, std::sync::atomic::Ordering::Relaxed);

    let (tx, rx) = crossbeam_channel::unbounded::<ChessMessage>();
    state.rx = Some(rx);

    spawn_reconnecting_subscription(base_url, game_id, tx, state.connected.clone(), rt);
}

fn spawn_reconnecting_subscription(
    base_url: String,
    game_id: String,
    tx: crossbeam_channel::Sender<ChessMessage>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
    rt: &tokio::runtime::Handle,
) {
    rt.spawn(async move {
        let mut backoff = MIN_BACKOFF;
        // Only true once we've actually been connected and lost it — gates
        // the "reconnected" notice so the very first connect of a match
        // (the common case, nothing wrong) stays quiet.
        let mut recovering = false;
        loop {
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

            // Run both streams concurrently on this task; if either drops,
            // reconnect both (simplest correct behavior — the backend
            // replays full history on resubscribe, so this is never a
            // silent gap, just a brief reconnect delay).
            let moves = sub.subscribe_moves().await;
            let chat = sub.subscribe_chat().await;
            let (moves_rx, chat_rx) = match (moves, chat) {
                (Ok((m, _)), Ok((c, _))) => (m, c),
                (Err(e), _) | (_, Err(e)) => {
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
                            Ok(m) => { let _ = tx.send(m); }
                            Err(_) => break,
                        }
                    }
                    msg = chat_rx.recv() => {
                        match msg {
                            Ok(m) => { let _ = tx.send(m); }
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
    });
}

pub fn drain_braid_messages(
    state: Res<BraidTransportState>,
    session: Res<OnlineGameSession>,
    mut causal: ResMut<CausalChainState>,
    mut network_events: MessageWriter<NetworkEvent>,
    mut resign_events: MessageWriter<crate::game::events::ResignEvent>,
    mut chat_events: MessageWriter<OnlineChatMessage>,
) {
    let Some(rx) = &state.rx else {
        return;
    };
    let game_id_u64 =
        crate::multiplayer::network::online_game_session::numeric_game_id(&session.game_id);

    while let Ok(msg) = rx.try_recv() {
        // Advance the shared stream head to whatever the server actually
        // accepted, and drop the echo of our own publishes. The backend
        // broadcasts every accepted event to all subscribers *including the
        // publisher*, so without the `self_published` check we would re-apply
        // our own moves as if the opponent had sent them, and re-display our
        // own chat lines twice.
        let mut is_self_echo = false;
        if let Some(version) = version_of(&msg) {
            if let Ok(mut h) = state.heads.lock() {
                h.set_head(stream_of(&msg), version.clone());
                is_self_echo = h.self_published.remove(&version);
            }
        }
        if is_self_echo {
            continue;
        }

        match msg {
            ChessMessage::Move(payload) => {
                let version = braid_chess::version_hash(&payload.fen_after, payload.move_number);
                let already_seen = causal
                    .applied_versions
                    .get(&game_id_u64)
                    .is_some_and(|versions| versions.contains(&version))
                    || causal
                        .pending_versions
                        .get(&game_id_u64)
                        .is_some_and(|versions| versions.contains(&version));
                if already_seen {
                    continue; // already applied via gossip — see module doc comment
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

                // Mirrors `handle_network_events`'s gossip-side roster
                // building exactly (`systems.rs`) — this is the durable
                // fallback for the same real bug described at this
                // message's publish site (gossip alone can silently drop
                // SessionInfo before the P2P link establishes).
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

                // The roster update above only feeds move-signer
                // verification. `handle_session_info_from_network`
                // (multiplayer/systems.rs) is the only place
                // `SolanaIntegrationState::opponent_pubkey` gets set —
                // required before `bridge.rs` can finalize a game
                // on-chain — and it listens only for
                // `NetworkEvent::MessageReceived(NetworkMessage::SessionInfo)`.
                // Without re-emitting that event here too, a SessionInfo
                // that only arrives via Braid (because gossip dropped it —
                // the exact failure this dual-transport send exists for)
                // never reaches that handler, and both sides get stuck
                // logging "Opponent pubkey unavailable" forever at game
                // end, never settling on-chain.
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
            }
            // OfferDraw/AcceptDraw/DeclineDraw/Clock/EngineAnalysis aren't
            // published through this transport (draw offers stay
            // gossip-only for now; Clock/EngineAnalysis are separate Braid
            // streams this module doesn't subscribe to).
            _ => {}
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

pub struct BraidTransportPlugin;

impl Plugin for BraidTransportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BraidTransportState>()
            .add_systems(Update, (sync_braid_subscription, drain_braid_messages));
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

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
            braid_chess::version_hash("session:wallet1", 0)
        );
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
