use bevy::prelude::*;
use braid_chess::MovePayload;
use tracing::{info, warn};

use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::types::{is_online_game_mode, NetworkEvent, OnlineStartBarrier};

#[derive(Resource)]
pub struct OnlineGameSession {
    pub base_url: String,
    pub game_id: String,
    pub active: bool,
    pub next_move_number: u32,
    pub next_nonce: u64,
    pub wager_amount: f64,
    pub last_published_move_version: String,
    /// The last move played by either side; a move's authorisation covers the
    /// move it answers.
    pub last_move_uci: Option<String>,
}

impl Default for OnlineGameSession {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            game_id: String::new(),
            active: false,
            next_move_number: 1,
            next_nonce: 1,
            wager_amount: 0.0,
            last_published_move_version: "0".to_string(),
            last_move_uci: None,
        }
    }
}

pub fn numeric_game_id(game_id: &str) -> u64 {
    if let Ok(n) = game_id.parse::<u64>() {
        return n;
    }
    use sha2::Digest;
    let digest = sha2::Sha256::digest(game_id.as_bytes());
    let mut prefix = [0; 8];
    prefix.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(prefix)
}

impl OnlineGameSession {
    pub fn reset(&mut self) {
        self.base_url.clear();
        self.game_id.clear();
        self.active = false;
        self.next_move_number = 1;
        self.next_nonce = 1;
        self.wager_amount = 0.0;
        self.last_published_move_version = "0".to_string();
        self.last_move_uci = None;
    }

    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty() && !self.game_id.is_empty()
    }
}

#[derive(Message, Debug, Clone)]
pub struct PublishOnlineChat {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
}

/// Ask the transport to reconnect now instead of waiting out its backoff: the
/// player pressed "Refresh state" on the connection-lost banner.
#[derive(Message, Debug, Clone, Copy)]
pub struct RefreshGameState;

#[derive(Message, Debug, Clone)]
pub struct OnlineChatMessage {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
}

pub struct OnlineGameSessionPlugin;

impl Plugin for OnlineGameSessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OnlineGameSession>()
            .init_resource::<OnlineStartBarrier>()
            .add_message::<PublishOnlineChat>()
            .add_message::<OnlineChatMessage>()
            .add_message::<RefreshGameState>()
            .add_systems(
                Update,
                drain_chat_messages
                    .after(crate::multiplayer::systems::handle_pong)
                    .in_set(crate::core::schedule::ReceiveSet::Game),
            )
            .add_systems(
                Update,
                (
                    announce_game_readiness,
                    retry_pending_writes,
                    publish_local_move,
                    publish_clock_on_move,
                    handle_publish_chat,
                    handle_publish_game_actions,
                    drain_clock_to_spectator,
                )
                    .chain()
                    .in_set(crate::core::schedule::SendSet::Moves),
            );
        app.add_systems(
            OnExit(crate::core::state::GameState::InGame),
            reset_start_barrier,
        );
        #[cfg(feature = "solana")]
        app.add_systems(
            Update,
            // After the whole publish chain, so a joining peer's snapshot includes it.
            broadcast_snapshot_to_new_peer
                .after(drain_clock_to_spectator)
                .in_set(crate::core::schedule::SendSet::Moves),
        );
    }
}

fn reset_start_barrier(mut barrier: ResMut<OnlineStartBarrier>) {
    barrier.reset(0);
}

/// How often an unacknowledged `GameReady` is sent again, and how many times at most.
const READY_RESEND_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
const READY_RESEND_LIMIT: u32 = 150;

#[derive(Default)]
pub struct ReadyResend {
    game_id: u64,
    /// `Time::elapsed` when the last announcement went out.
    last_sent: Option<std::time::Duration>,
    attempts: u32,
}

/// `GameReady` is a single gossip message. If the opponent was not yet subscribed when it
/// went out, nothing else would ever deliver it and neither side could start.
fn should_resend_ready(
    barrier: &OnlineStartBarrier,
    since_last_send: Option<std::time::Duration>,
    attempts: u32,
) -> bool {
    barrier.ready_sent
        && !barrier.remote_started
        && attempts < READY_RESEND_LIMIT
        && since_last_send.is_none_or(|elapsed| elapsed >= READY_RESEND_INTERVAL)
}

pub fn announce_game_readiness(
    session: Res<OnlineGameSession>,
    mut barrier: ResMut<OnlineStartBarrier>,
    mut resend: Local<ReadyResend>,
    time: Res<Time>,
    pieces: Query<&crate::rendering::pieces::Piece>,
    network_state: Res<crate::multiplayer::OnlineNetworkState>,
    game_mode: Res<crate::core::GameMode>,
    #[cfg(feature = "solana")] wallet: Option<Res<crate::multiplayer::solana::addon::SolanaWallet>>,
    #[cfg(feature = "solana")] solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    #[cfg(feature = "solana")] sync: Option<Res<crate::multiplayer::solana::addon::SolanaGameSync>>,
    #[cfg(feature = "solana")] resolver: Option<
        Res<crate::multiplayer::rollup::magicblock::MagicBlockResolver>,
    >,
) {
    if !is_online_game_mode(*game_mode) || !session.is_configured() {
        return;
    }

    let game_id = numeric_game_id(&session.game_id);
    if barrier.game_id != game_id {
        barrier.reset(game_id);
    }
    if resend.game_id != game_id {
        *resend = ReadyResend {
            game_id,
            ..ReadyResend::default()
        };
    }
    if pieces.iter().count() < 32 {
        return;
    }
    let resending = barrier.ready_sent;
    if resending
        && !should_resend_ready(
            &barrier,
            resend
                .last_sent
                .map(|sent| time.elapsed().saturating_sub(sent)),
            resend.attempts,
        )
    {
        return;
    }

    #[cfg(feature = "solana")]
    let player_pubkey = {
        let on_chain = sync.as_ref().is_some_and(|sync| sync.game_id.is_some());
        let Some(wallet) = ready_pubkey(
            wallet.as_ref().and_then(|wallet| wallet.pubkey),
            solana_state.as_ref().and_then(|state| state.wallet_pubkey),
            on_chain,
        ) else {
            return;
        };
        if sync.as_ref().is_some_and(|sync| sync.requires_delegation)
            && !resolver
                .as_ref()
                .is_some_and(|resolver| resolver.is_delegated())
        {
            return;
        }
        wallet
    };
    #[cfg(not(feature = "solana"))]
    let player_pubkey = crate::multiplayer::network::protocol::Pubkey::default();

    barrier.local_ready = true;
    let Some(tx) = &network_state.message_sender else {
        return;
    };
    let ready_token = format!("game-{game_id}");
    if tx
        .send(NetworkMessage::GameReady {
            game_id,
            player_pubkey,
            ready_token,
        })
        .is_ok()
    {
        resend.last_sent = Some(time.elapsed());
        if resending {
            resend.attempts += 1;
            if resend.attempts == READY_RESEND_LIMIT {
                warn!("[START] Opponent never acknowledged readiness for game {game_id}; giving up re-sending");
            } else {
                info!(
                    "[START] Re-sent readiness for game {game_id} (attempt {}); opponent has not confirmed",
                    resend.attempts
                );
            }
            return;
        }
        barrier.ready_sent = true;
        info!("[START] Local player ready for game {game_id}; waiting for opponent");
        if barrier.remote_ready
            && !barrier.start_sent
            && tx
                .send(NetworkMessage::GameStartConfirmed {
                    game_id,
                    start_token: format!("game-{game_id}"),
                })
                .is_ok()
        {
            barrier.start_sent = true;
            barrier.start_confirmed = true;
            info!("[START] Sent synchronized start for game {game_id}");
        }
    }
}

/// The pubkey announced in `GameReady`. A wallet connected through the wallet bridge
/// is recorded in `SolanaIntegrationState` only, so both sources are consulted. A game
/// that is not on-chain does not need a wallet: skipping the announcement for a player
/// without one would leave both clients waiting on the start barrier forever.
#[cfg(feature = "solana")]
fn ready_pubkey(
    wallet: Option<solana_sdk::pubkey::Pubkey>,
    integration: Option<solana_sdk::pubkey::Pubkey>,
    on_chain: bool,
) -> Option<solana_sdk::pubkey::Pubkey> {
    wallet
        .or(integration)
        .or_else(|| (!on_chain).then(solana_sdk::pubkey::Pubkey::default))
}

#[cfg(all(test, feature = "solana"))]
mod ready_pubkey_tests {
    use super::ready_pubkey;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn a_bridge_connected_wallet_is_announced() {
        let bridged = Pubkey::new_unique();
        assert_eq!(ready_pubkey(None, Some(bridged), true), Some(bridged));
        assert_eq!(ready_pubkey(None, Some(bridged), false), Some(bridged));
    }

    #[test]
    fn the_wallet_resource_wins_when_both_are_set() {
        let (wallet, bridged) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert_eq!(
            ready_pubkey(Some(wallet), Some(bridged), true),
            Some(wallet)
        );
    }

    #[test]
    fn a_casual_game_starts_without_a_wallet_but_an_on_chain_game_does_not() {
        assert_eq!(ready_pubkey(None, None, false), Some(Pubkey::default()));
        assert_eq!(ready_pubkey(None, None, true), None);
    }
}

#[cfg(test)]
mod ready_resend_tests {
    use super::*;
    use std::time::Duration;

    fn sent() -> OnlineStartBarrier {
        OnlineStartBarrier {
            game_id: 1,
            local_ready: true,
            ready_sent: true,
            ..OnlineStartBarrier::default()
        }
    }

    #[test]
    fn nothing_is_resent_before_the_first_announcement() {
        let barrier = OnlineStartBarrier::default();
        assert!(!should_resend_ready(&barrier, None, 0));
    }

    #[test]
    fn an_unacknowledged_ready_is_resent_once_the_interval_has_passed() {
        let barrier = sent();
        assert!(!should_resend_ready(
            &barrier,
            Some(Duration::from_millis(500)),
            0
        ));
        assert!(should_resend_ready(
            &barrier,
            Some(READY_RESEND_INTERVAL),
            0
        ));
    }

    #[test]
    fn resending_stops_once_the_opponent_has_confirmed() {
        let mut barrier = sent();
        barrier.remote_started = true;
        assert!(!should_resend_ready(
            &barrier,
            Some(READY_RESEND_INTERVAL * 5),
            3
        ));
    }

    #[test]
    fn resending_is_bounded() {
        let barrier = sent();
        assert!(should_resend_ready(&barrier, None, READY_RESEND_LIMIT - 1));
        assert!(!should_resend_ready(&barrier, None, READY_RESEND_LIMIT));
    }
}

#[cfg(feature = "solana")]
fn braid_sender_identity(
    solana_state: Option<
        &Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    game_sync: Option<&Res<crate::multiplayer::solana::addon::SolanaGameSync>>,
    node_b58: &str,
) -> String {
    let on_chain_game = game_sync.and_then(|s| s.game_id).is_some();
    if on_chain_game {
        solana_state
            .and_then(|s| s.wallet_pubkey)
            .map(|pk| pk.to_string())
            .unwrap_or_else(|| node_b58.to_string())
    } else {
        node_b58.to_string()
    }
}

#[cfg(feature = "solana")]
fn braid_session_token(
    game_sync: Option<&Res<crate::multiplayer::solana::addon::SolanaGameSync>>,
) -> String {
    game_sync
        .and_then(|sync| sync.session_pubkey)
        .map(|key| key.to_string())
        .unwrap_or_default()
}

#[cfg(not(feature = "solana"))]
fn braid_sender_identity(node_b58: &str) -> String {
    node_b58.to_string()
}

fn retry_pending_writes(
    session: Res<OnlineGameSession>,
    network: Res<crate::multiplayer::OnlineNetworkState>,
    braid: Res<crate::multiplayer::network::braid_transport::BraidTransportState>,
    time: Res<Time>,
    mut refresh: MessageReader<RefreshGameState>,
    mut delay: Local<f32>,
    #[cfg(feature = "solana")] solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    #[cfg(feature = "solana")] game_sync: Option<
        Res<crate::multiplayer::solana::addon::SolanaGameSync>,
    >,
) {
    if !session.active || !session.is_configured() {
        *delay = 0.;
        return;
    }
    // A refresh retries the outbox now rather than at the next 30 s tick.
    if refresh.read().count() > 0 {
        *delay = 0.;
    }
    *delay -= time.delta_secs();
    if *delay > 0. {
        return;
    }
    *delay = 30.;
    let node = network
        .node_id
        .as_ref()
        .map(|id| bs58::encode(id.as_bytes()).into_string())
        .unwrap_or_default();
    #[cfg(feature = "solana")]
    let sender = braid_sender_identity(solana_state.as_ref(), game_sync.as_ref(), &node);
    #[cfg(not(feature = "solana"))]
    let sender = braid_sender_identity(&node);
    #[cfg(feature = "solana")]
    let token = braid_session_token(game_sync.as_ref());
    #[cfg(not(feature = "solana"))]
    let token = String::new();
    if !sender.is_empty() {
        crate::multiplayer::network::braid_transport::retry_pending(
            session.base_url.clone(),
            session.game_id.clone(),
            sender,
            token,
            braid.heads(),
        );
    }
}

fn publish_local_move(
    mut session: ResMut<OnlineGameSession>,
    mut move_events: MessageReader<crate::game::events::MoveMadeEvent>,
    network_state: Res<crate::multiplayer::OnlineNetworkState>,
    p2p_conn: Res<crate::multiplayer::network::p2p::P2PConnectionState>,
    braid: Res<crate::multiplayer::network::braid_transport::BraidTransportState>,
    mut causal: ResMut<crate::multiplayer::types::CausalChainState>,
    #[cfg(feature = "solana")] solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    #[cfg(feature = "solana")] game_sync: Option<
        Res<crate::multiplayer::solana::addon::SolanaGameSync>,
    >,
) {
    if !session.is_configured() {
        move_events.clear();
        return;
    }
    for event in move_events.read() {
        let from_file = (b'a' + event.from.0) as char;
        let from_rank = event.from.1 + 1;
        let to_file = (b'a' + event.to.0) as char;
        let to_rank = event.to.1 + 1;
        let mut uci = format!("{}{}{}{}", from_file, from_rank, to_file, to_rank);
        if let Some(promo) = event.promotion {
            let promo_char = match promo {
                crate::game::components::PieceType::Queen => 'q',
                crate::game::components::PieceType::Rook => 'r',
                crate::game::components::PieceType::Bishop => 'b',
                crate::game::components::PieceType::Knight => 'n',
                _ => 'q',
            };
            uci.push(promo_char);
        }
        if event.remote {
            session.last_move_uci = Some(uci);
            continue;
        }
        let game_id_u64 = numeric_game_id(&session.game_id);
        let move_number = session.next_move_number;
        let nonce = session.next_nonce;

        // Lets the opponent relay this move to the backend on our behalf. It
        // travels in the Braid log only: the gossip wire format is unchanged,
        // so clients of different versions still decode each other's moves.
        #[cfg(feature = "solana")]
        let move_auth = solana_state
            .as_ref()
            .and_then(|state| state.wallet_pubkey)
            .and_then(|wallet| {
                crate::multiplayer::network::move_auth::sign_move(
                    &wallet.to_string(),
                    game_id_u64,
                    &uci,
                    &event.next_fen,
                    session.last_move_uci.as_deref(),
                )
            });
        #[cfg(not(feature = "solana"))]
        let move_auth: Option<String> = None;

        // Causal chain fields.
        let parent_version = session.last_published_move_version.clone();
        let new_version = braid_chess::version_hash(&event.next_fen, move_number);
        causal
            .applied_versions
            .entry(game_id_u64)
            .or_default()
            .insert(new_version.clone());
        session.last_published_move_version = new_version.clone();

        // Use the gossip signing key, not the Iroh node ID, for signer_pubkey. Gossip
        // verifies it against the participant roster; Braid moves are authenticated and
        // causally validated by the backend.
        let signer_pubkey = network_state
            .session_signing_key
            .map(|seed| {
                use ed25519_dalek::SigningKey;
                SigningKey::from_bytes(&seed)
                    .verifying_key()
                    .to_bytes()
                    .to_vec()
            })
            .unwrap_or_default();

        // Use move_number as the per-agent sequence number (monotonic, unique).
        let seq = move_number as u64;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let _ = p2p_conn.player_color; // referenced for completeness; color is in the FEN

        let node_b58 = network_state
            .node_id
            .as_ref()
            .map(|id| bs58::encode(id.as_bytes()).into_string())
            .unwrap_or_default();

        // Use wallet identity for on-chain games and Iroh identity for casual JOIN_ACK rosters.
        #[cfg(feature = "solana")]
        let braid_sender_identity =
            braid_sender_identity(solana_state.as_ref(), game_sync.as_ref(), &node_b58);
        #[cfg(feature = "solana")]
        let braid_session_token = braid_session_token(game_sync.as_ref());
        #[cfg(not(feature = "solana"))]
        let braid_sender_identity = braid_sender_identity(&node_b58);
        #[cfg(not(feature = "solana"))]
        let braid_session_token = String::new();

        // Also persist moves through Braid; receivers deduplicate transport copies
        // using applied_versions.
        crate::multiplayer::network::braid_transport::publish_move(
            session.base_url.clone(),
            session.game_id.clone(),
            braid_sender_identity,
            braid_session_token,
            MovePayload::from_uci(
                uci.clone(),
                event.next_fen.clone(),
                move_number,
                node_b58.clone(),
            )
            .with_auth(move_auth),
            new_version,
            braid.heads(),
        );

        let msg = NetworkMessage::Move {
            game_id: game_id_u64,
            turn: move_number as u16,
            move_uci: uci.clone(),
            next_fen: event.next_fen.clone(),
            nonce,
            timestamp_ms,
            signer_pubkey,
            seq,
            parent_version,
        };

        if let Some(ref tx) = network_state.message_sender {
            if let Err(e) = tx.send(msg) {
                warn!(
                    "[online-session] Move send failed (game {game_id_u64}, turn {move_number}): {e}"
                );
            } else {
                info!(
                    "[online-session] Move sent (game {game_id_u64}, turn {move_number}, uci {uci}) via gossip+braid"
                );
            }
        } else {
            warn!(
                "[online-session] No gossip sender for move (game {game_id_u64}, turn {move_number}, uci {uci}); relying on Braid publish"
            );
        }

        session.next_move_number = session.next_move_number.saturating_add(1);
        session.next_nonce = session.next_nonce.saturating_add(1);
        session.last_move_uci = Some(uci);
    }
}

pub fn start_session(
    session: &mut OnlineGameSession,
    base_url: String,
    game_id: String,
    wager_amount: f64,
    network_state: &crate::multiplayer::OnlineNetworkState,
) {
    if session.active && session.base_url == base_url && session.game_id == game_id {
        return;
    }
    session.reset();
    session.base_url = base_url;
    session.game_id = game_id.clone();
    session.wager_amount = wager_amount;
    session.active = true;

    if let Some(ref sub_tx) = network_state.subscription_sender {
        let topic = format!(
            "{}/{}",
            crate::multiplayer::systems::GAME_TOPIC,
            numeric_game_id(&game_id)
        );
        if let Err(e) = sub_tx.send(topic) {
            warn!("[online-session] Failed to subscribe to iroh topic: {e}");
        } else {
            info!("[online-session] Subscribed to iroh gossip for game {game_id}");
        }
    }
}

fn handle_publish_chat(
    session: Res<OnlineGameSession>,
    mut reader: MessageReader<PublishOnlineChat>,
    network_state: Res<crate::multiplayer::OnlineNetworkState>,
    braid: Res<crate::multiplayer::network::braid_transport::BraidTransportState>,
    #[cfg(feature = "solana")] solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    #[cfg(feature = "solana")] game_sync: Option<
        Res<crate::multiplayer::solana::addon::SolanaGameSync>,
    >,
) {
    if !session.is_configured() {
        reader.clear();
        return;
    }
    for event in reader.read() {
        let game_id_u64 = numeric_game_id(&session.game_id);
        let msg = NetworkMessage::Chat {
            game_id: game_id_u64,
            player: event.player.clone(),
            text: event.text.clone(),
            timestamp_ms: event.timestamp_ms,
        };
        let node_b58 = network_state
            .node_id
            .as_ref()
            .map(|id| bs58::encode(id.as_bytes()).into_string())
            .unwrap_or_default();
        // Chat is participant-checked server-side just like moves — see
        // `braid_sender_identity`.
        #[cfg(feature = "solana")]
        let braid_sender_identity =
            braid_sender_identity(solana_state.as_ref(), game_sync.as_ref(), &node_b58);
        #[cfg(feature = "solana")]
        let braid_session_token = braid_session_token(game_sync.as_ref());
        #[cfg(not(feature = "solana"))]
        let braid_sender_identity = braid_sender_identity(&node_b58);
        #[cfg(not(feature = "solana"))]
        let braid_session_token = String::new();
        crate::multiplayer::network::braid_transport::publish_chat(
            session.base_url.clone(),
            session.game_id.clone(),
            braid_sender_identity,
            braid_session_token,
            event.player.clone(),
            event.text.clone(),
            event.timestamp_ms,
            braid.heads(),
        );
        if let Some(ref tx) = network_state.message_sender {
            if let Err(e) = tx.send(msg) {
                warn!("[online-session] Chat send failed: {e}");
            }
        }
    }
}

/// Put our draw offers, acceptances, refusals and resignations on the Braid log. Gossip
/// tells a connected opponent at once; the log is what reaches one who is offline, and
/// what a later resume reads to learn the game was already over.
fn handle_publish_game_actions(
    session: Res<OnlineGameSession>,
    mut offers: MessageReader<crate::game::events::DrawOfferEvent>,
    mut responses: MessageReader<crate::game::events::DrawResponseEvent>,
    mut resigns: MessageReader<crate::game::events::ResignEvent>,
    engine: Res<crate::engine::board_state::ChessEngine>,
    network_state: Res<crate::multiplayer::OnlineNetworkState>,
    braid: Res<crate::multiplayer::network::braid_transport::BraidTransportState>,
    #[cfg(feature = "solana")] solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    #[cfg(feature = "solana")] game_sync: Option<
        Res<crate::multiplayer::solana::addon::SolanaGameSync>,
    >,
) {
    use braid_chess::ChessMessage;

    if !session.is_configured() {
        offers.clear();
        responses.clear();
        resigns.clear();
        return;
    }
    let ply = crate::game::systems::network_move::position_ply(&engine);
    let at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64);

    let mut outgoing: Vec<ChessMessage> = Vec::new();
    for event in offers.read().filter(|event| !event.remote) {
        outgoing.push(ChessMessage::OfferDraw {
            player: event.player.clone(),
            ply,
            at_ms,
        });
    }
    for event in responses.read().filter(|event| !event.remote) {
        outgoing.push(if event.accepted {
            ChessMessage::AcceptDraw {
                player: event.player.clone(),
                ply,
                at_ms,
            }
        } else {
            ChessMessage::DeclineDraw {
                player: event.player.clone(),
                ply,
                at_ms,
            }
        });
    }
    let resigning: Vec<String> = resigns
        .read()
        .filter(|event| !event.remote)
        .map(|event| {
            if event.winner == "white" {
                "black"
            } else {
                "white"
            }
            .to_string()
        })
        .collect();
    if outgoing.is_empty() && resigning.is_empty() {
        return;
    }

    let node_b58 = network_state
        .node_id
        .as_ref()
        .map(|id| bs58::encode(id.as_bytes()).into_string())
        .unwrap_or_default();
    // Participant-checked server-side like moves — see `braid_sender_identity`.
    #[cfg(feature = "solana")]
    let braid_sender_identity =
        braid_sender_identity(solana_state.as_ref(), game_sync.as_ref(), &node_b58);
    #[cfg(feature = "solana")]
    let braid_session_token = braid_session_token(game_sync.as_ref());
    #[cfg(not(feature = "solana"))]
    let braid_sender_identity = braid_sender_identity(&node_b58);
    #[cfg(not(feature = "solana"))]
    let braid_session_token = String::new();
    for message in outgoing {
        crate::multiplayer::network::braid_transport::publish_draw(
            session.base_url.clone(),
            session.game_id.clone(),
            braid_sender_identity.clone(),
            braid_session_token.clone(),
            message,
            braid.heads(),
        );
    }
    for player in resigning {
        crate::multiplayer::network::braid_transport::publish_resign(
            session.base_url.clone(),
            session.game_id.clone(),
            braid_sender_identity.clone(),
            braid_session_token.clone(),
            player,
            braid.heads(),
        );
    }
}

fn drain_chat_messages(
    session: Res<OnlineGameSession>,
    mut network_events: MessageReader<NetworkEvent>,
    mut writer: MessageWriter<OnlineChatMessage>,
) {
    if !session.is_configured() {
        return;
    }
    let game_id_u64 = numeric_game_id(&session.game_id);
    for ev in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::Chat {
            game_id,
            player,
            text,
            timestamp_ms,
        }) = ev
        {
            if *game_id == game_id_u64 {
                writer.write(OnlineChatMessage {
                    player: player.clone(),
                    text: text.clone(),
                    timestamp_ms: *timestamp_ms,
                });
            }
        }
    }
}

fn drain_clock_to_spectator(
    session: Res<OnlineGameSession>,
    mut network_events: MessageReader<NetworkEvent>,
    mut clock: ResMut<crate::multiplayer::spectator::SpectatorClockState>,
    time: Res<Time>,
) {
    if !session.is_configured() {
        return;
    }
    let game_id_u64 = numeric_game_id(&session.game_id);
    for ev in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::Clock {
            game_id,
            white_ms,
            black_ms,
            ..
        }) = ev
        {
            if *game_id == game_id_u64 {
                clock.white_ms = *white_ms;
                clock.black_ms = *black_ms;
                clock.last_update_secs = time.elapsed_secs_f64();
            }
        }
    }
}

fn publish_clock_on_move(
    mut move_events: MessageReader<crate::game::events::MoveMadeEvent>,
    session: Res<OnlineGameSession>,
    game_timer: Option<Res<crate::game::resources::turn::timer::GameTimer>>,
    network_state: Res<crate::multiplayer::OnlineNetworkState>,
) {
    if !session.is_configured() {
        move_events.clear();
        return;
    }
    let Some(timer) = game_timer else {
        move_events.clear();
        return;
    };

    for event in move_events.read() {
        if event.remote {
            continue;
        }
        let game_id_u64 = numeric_game_id(&session.game_id);
        let white_ms = (timer.white_time_left * 1000.0) as u64;
        let black_ms = (timer.black_time_left * 1000.0) as u64;
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        if let Some(ref tx) = network_state.message_sender {
            let _ = tx.send(NetworkMessage::Clock {
                game_id: game_id_u64,
                white_ms,
                black_ms,
                timestamp_ms,
            });
        }
    }
}

#[cfg(feature = "solana")]
fn broadcast_snapshot_to_new_peer(
    session: Res<OnlineGameSession>,
    mut network_events: MessageReader<NetworkEvent>,
    network_state: Res<crate::multiplayer::OnlineNetworkState>,
    rollup_manager: Res<crate::multiplayer::EphemeralRollupManager>,
) {
    if !session.is_configured() || session.last_published_move_version == "0" {
        network_events.clear();
        return;
    }
    let game_id = numeric_game_id(&session.game_id);

    let mut peer_joined = false;
    for event in network_events.read() {
        if let NetworkEvent::PeerConnected(_) = event {
            peer_joined = true;
        }
    }
    if !peer_joined {
        return;
    }

    let committed_fen = rollup_manager.committed_fen.clone();
    let msg_tx = network_state.message_sender.clone();

    crate::core::net_task::spawn_detached("online_game_session", move || {
        use crate::multiplayer::vps_client;
        let Ok(moves) = vps_client::fetch_move_log(game_id) else {
            return;
        };
        // Snapshot FEN, history and head must describe the same durable position.
        let fen = moves
            .last()
            .map(|payload| payload.fen_after.clone())
            .unwrap_or(committed_fen);
        let head_version = moves
            .last()
            .map(|payload| braid_chess::version_hash(&payload.fen_after, payload.move_number))
            .unwrap_or_else(|| "0".into());
        let move_payloads: Vec<String> = moves
            .iter()
            .filter_map(|m| serde_json::to_string(m).ok())
            .collect();

        info!(
            "[online-session] New peer joined — sending GameSnapshot for game {} ({} moves)",
            game_id,
            move_payloads.len()
        );

        if let Some(tx) = msg_tx {
            let _ = tx.send(NetworkMessage::GameSnapshot {
                game_id,
                fen,
                move_payloads,
                head_version,
            });
        }
    });
}

pub fn make_move_payload(
    session: &OnlineGameSession,
    uci: String,
    fen_after: String,
    player: String,
) -> MovePayload {
    MovePayload::from_uci(
        uci,
        fen_after,
        session.next_move_number.saturating_sub(1),
        player,
    )
}

#[cfg(test)]
mod movement_publication_tests {
    use super::*;
    use crate::game::events::MoveMadeEvent;
    use crate::multiplayer::network::{
        braid_transport::{open_journal, BraidTransportState},
        p2p::P2PConnectionState,
    };
    use crate::multiplayer::types::{CausalChainState, OnlineNetworkState};
    use crate::rendering::pieces::{Piece, PieceColor, PieceType};
    use std::fs::OpenOptions;
    use std::time::Duration;

    #[test]
    fn one_gameplay_move_publishes_once_to_each_transport_despite_motion_and_view_changes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (fps, enabled) in [(30, true), (60, true), (144, true), (60, false)] {
            let base = "http://127.0.0.1:1".to_string();
            let id = format!(
                "movement-publication-{}-{}-{enabled}-{}",
                std::process::id(),
                fps,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos()
            );
            let journal = open_journal(&base, &id)?;
            // Exercise the real durable enqueue path. Holding the worker lock
            // keeps this deterministic and prevents any HTTP publication.
            let worker_lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(journal.directory().join("publisher.moves.lock"))?;
            worker_lock.try_lock()?;
            let mut app = App::new();
            app.init_resource::<Time>()
                .init_resource::<crate::motion::BoardEpoch>()
                .init_resource::<BraidTransportState>()
                .init_resource::<P2PConnectionState>()
                .init_resource::<CausalChainState>()
                .init_resource::<crate::game::view_mode::ViewMode>();
            app.insert_resource(crate::motion::gameplay::MotionSettings {
                enabled,
                ..Default::default()
            });
            app.insert_resource(OnlineGameSession {
                base_url: base,
                game_id: id,
                active: true,
                ..Default::default()
            });
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            app.insert_resource(OnlineNetworkState {
                message_sender: Some(tx),
                ..Default::default()
            });
            app.add_message::<MoveMadeEvent>();
            app.add_systems(
                Update,
                (crate::motion::advance_piece_motions, publish_local_move).chain(),
            );
            let entity = app
                .world_mut()
                .spawn((
                    Piece::new(PieceColor::White, PieceType::Pawn, 4, 3),
                    crate::game::components::HasMoved::default(),
                ))
                .id();
            use bevy::ecs::system::Command;
            crate::motion::StartPieceMotion::gameplay(entity, (4, 1), (4, 3))
                .apply(app.world_mut());
            let event = MoveMadeEvent {
                from: (4, 1),
                to: (4, 3),
                player: "White".into(),
                piece_type: PieceType::Pawn,
                captured_piece: None,
                promotion: None,
                remote: false,
                game_id: None,
                next_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".into(),
            };
            app.world_mut().write_message(event.clone());
            app.world_mut().write_message(MoveMadeEvent {
                remote: true,
                ..event
            });
            for frame in 0..fps {
                if frame == fps / 4 {
                    *app.world_mut()
                        .resource_mut::<crate::game::view_mode::ViewMode>() =
                        crate::game::view_mode::ViewMode::Standard2D;
                }
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(Duration::from_secs_f32(1.0 / fps as f32));
                app.update();
            }
            assert!(
                matches!(rx.try_recv()?, NetworkMessage::Move { ref move_uci, .. } if move_uci == "e2e4")
            );
            assert!(rx.try_recv().is_err());
            assert_eq!(journal.pending()?.len(), 1);
            assert_eq!(
                app.world().resource::<OnlineGameSession>().next_move_number,
                2
            );
            assert_eq!(app.world().resource::<OnlineGameSession>().next_nonce, 2);
        }
        Ok(())
    }
}
