use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::states::{GameMode, GameState};
use crate::game::events::NetworkMoveEvent;
#[cfg(feature = "solana")]
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::traits::{Message, MessageReader, MessageWriter};
use crate::multiplayer::TokioRuntime;
use bevy::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct SpectatorMatchDetails {
    pub tournament_name: Option<String>,
    pub round: Option<u8>,
    pub white: Option<String>,
    pub black: Option<String>,
}

#[derive(Resource, Default)]
pub struct SpectatorMatchInfo(pub SpectatorMatchDetails);

#[derive(Message, Debug, Clone)]
pub struct SpectateViaLinkEvent {
    pub game_id: String,
    pub details: Option<SpectatorMatchDetails>,
    pub tournament_id: Option<u64>,
    pub playlist: Vec<SpectatorPlaylistEntry>,
}

pub fn parse_spectate_link(url: &str) -> Option<String> {
    url.strip_prefix("xfchess://spectate/")
        .filter(|id| !id.is_empty())
        .map(|id| id.to_string())
}

pub fn make_spectate_link(game_id: &str) -> String {
    format!("xfchess://spectate/{}", game_id)
}

pub fn feed_is_delayed(delay_secs: u64) -> bool {
    delay_secs > 0
}

pub type LiveFeedBuffer = Arc<Mutex<Vec<String>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectatorPlaylistEntry {
    pub game_id: String,
    pub white: String,
    pub black: String,
    pub round: u8,
}

#[derive(Resource, Default)]
pub struct SpectatorSession {
    pub game_id: Option<String>,
    pub tournament_id: Option<u64>,
    pub applied_move_count: usize,
    pub poll_timer: f32,
    pub pending_moves: Vec<String>,
    pub delayed: bool,
    pub delay_checked: bool,
    pub delay_result: Option<Arc<Mutex<Option<u64>>>>,
    pub live_feed: Option<LiveFeedBuffer>,
    pub live_subscribed: bool,
    pub live_cancel: Option<Arc<AtomicBool>>,
    pub playlist: Vec<SpectatorPlaylistEntry>,
    pub playlist_index: usize,
}

impl SpectatorSession {
    pub const POLL_INTERVAL: f32 = 2.0;

    pub fn is_live_subscribed(&self) -> bool {
        self.live_feed.is_some()
    }

    pub fn end_session(&mut self) {
        if let Some(flag) = self.live_cancel.take() {
            flag.store(true, Ordering::Relaxed);
        }
        self.game_id = None;
        self.applied_move_count = 0;
        self.poll_timer = 0.0;
        self.pending_moves.clear();
        self.delayed = true;
        self.delay_checked = false;
        self.delay_result = None;
        self.live_feed = None;
        self.live_subscribed = false;
    }

    pub fn begin_session(&mut self, game_id: String, tournament_id: Option<u64>) {
        self.end_session();
        self.playlist_index = self
            .playlist
            .iter()
            .position(|e| e.game_id == game_id)
            .unwrap_or(0);
        self.game_id = Some(game_id);
        self.tournament_id = tournament_id;
        self.delayed = true;
        self.delay_checked = false;
    }

    pub fn leave(&mut self) {
        self.end_session();
        self.tournament_id = None;
        self.playlist.clear();
        self.playlist_index = 0;
    }

    pub fn sibling(&self, offset: isize) -> Option<&SpectatorPlaylistEntry> {
        if self.playlist.len() < 2 {
            return None;
        }
        let len = self.playlist.len() as isize;
        let idx = (self.playlist_index as isize + offset).rem_euclid(len);
        self.playlist.get(idx as usize)
    }
}

#[derive(Resource, Default)]
pub struct SpectatorClockState {
    pub white_ms: u64,
    pub black_ms: u64,
    pub white_to_move: bool,
    pub last_update_secs: f64,
}

pub struct SpectatorPlugin;

impl Plugin for SpectatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorSession>()
            .init_resource::<SpectatorClockState>()
            .init_resource::<SpectatorMatchInfo>()
            .add_message::<SpectateViaLinkEvent>()
            .add_systems(
                Update,
                (
                    handle_spectate_link,
                    resolve_spectator_delay,
                    tick_spectator_poll,
                    drain_live_feed,
                    dispatch_pending_spectator_moves,
                    toggle_clock_side_on_move,
                ),
            );
        #[cfg(feature = "solana")]
        app.add_systems(
            Update,
            (apply_braid_resync_to_spectator, tick_spectator_clock),
        );
    }
}

fn handle_spectate_link(
    mut events: MessageReader<SpectateViaLinkEvent>,
    mut session: ResMut<SpectatorSession>,
    mut match_info: ResMut<SpectatorMatchInfo>,
    mut game_mode: ResMut<GameMode>,
    mut next_state: ResMut<NextState<GameState>>,
    tokio: Res<TokioRuntime>,
) {
    for ev in events.read() {
        info!("[spectator] Starting spectate for game {}", ev.game_id);
        match_info.0 = ev.details.clone().unwrap_or_default();
        // Fail safe: `begin_session` tears down any previous game's feed and
        // leaves this one marked delayed (HTTP-only) until the lookup confirms
        // otherwise. The live subscription is opened later, in
        // `resolve_spectator_delay`, only when the delay is 0.
        // A switch from the HUD carries the same playlist it came from; a
        // fresh spectate from a list supplies one. Either way, set it before
        // `begin_session` so the index resolves against the right list.
        if !ev.playlist.is_empty() {
            session.playlist = ev.playlist.clone();
        }
        session.begin_session(ev.game_id.clone(), ev.tournament_id);
        *game_mode = GameMode::Spectator;
        next_state.set(GameState::InGame);

        // Look up the game's broadcast delay off-thread.
        let slot = Arc::new(Mutex::new(None));
        session.delay_result = Some(slot.clone());
        let game_id = ev.game_id.clone();
        tokio.0.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                crate::multiplayer::network::vps::get_broadcast_delay(&game_id)
            })
            .await;
            // On any failure, leave the slot as a large delay (fail safe).
            let delay = match result {
                Ok(Ok(d)) => d,
                _ => u64::MAX,
            };
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(delay);
            }
        });
    }
}

fn resolve_spectator_delay(
    mut session: ResMut<SpectatorSession>,
    tokio: Res<TokioRuntime>,
    #[cfg(feature = "solana")] network_state: Option<Res<crate::multiplayer::OnlineNetworkState>>,
) {
    if session.delay_checked || session.game_id.is_none() {
        return;
    }
    let Some(slot) = session.delay_result.clone() else {
        return;
    };
    let delay = { slot.lock().ok().and_then(|g| *g) };
    let Some(delay) = delay else { return }; // still pending

    session.delayed = feed_is_delayed(delay);
    session.delay_checked = true;
    session.delay_result = None;

    if session.delayed {
        info!(
            "[spectator] game {:?} has a {}s broadcast delay — HTTP-only, no live gossip",
            session.game_id, delay
        );
        return;
    }

    info!(
        "[spectator] game {:?} is live (no delay) — opening the Braid feed",
        session.game_id
    );

    // The instant feed. Replaces the 2s poll for live games; the poll stands
    // down while `live_feed` is set (see `tick_spectator_poll`).
    if !session.live_subscribed {
        if let Some(game_id) = session.game_id.clone() {
            let (buffer, cancel) = spawn_live_feed(&game_id, &tokio);
            session.live_feed = Some(buffer);
            session.live_cancel = Some(cancel);
            session.live_subscribed = true;
        }
    }
    #[cfg(feature = "solana")]
    if let (Some(ref ns), Some(game_id)) = (
        network_state,
        session
            .game_id
            .as_ref()
            .map(|g| crate::multiplayer::network::online_game_session::numeric_game_id(g)),
    ) {
        // Subscribe to the game's iroh gossip topic so GameSnapshot arrives.
        if let Some(ref sub_tx) = ns.subscription_sender {
            let topic = format!("/xfchess-game/{}", game_id);
            let _ = sub_tx.send(topic);
        }
        // Request full move history from the active peer (since_version "0" = all).
        if let Some(ref msg_tx) = ns.message_sender {
            let _ = msg_tx.send(NetworkMessage::BraidResyncRequest {
                game_id,
                since_version: "0".to_string(),
            });
        }
    }
}

fn spawn_live_feed(game_id: &str, tokio: &TokioRuntime) -> (LiveFeedBuffer, Arc<AtomicBool>) {
    let buffer: LiveFeedBuffer = Arc::new(Mutex::new(Vec::new()));
    let cancel = Arc::new(AtomicBool::new(false));

    let url = format!(
        "{}/spectate/{}/moves",
        crate::multiplayer::network::vps::vps_base(),
        game_id
    );
    let buffer_task = buffer.clone();
    let cancel_task = cancel.clone();
    let game_id_owned = game_id.to_string();

    tokio.0.spawn(async move {
        use braid_chess::braid_http::types::BraidRequest;
        use braid_chess::braid_http::BraidClient;
        use braid_chess::ChessMessage;

        let client = match BraidClient::new() {
            Ok(c) => c,
            Err(e) => {
                warn!("[spectator] could not build Braid client: {e}");
                return;
            }
        };

        let mut subscription = match client.subscribe(&url, BraidRequest::new().subscribe()).await {
            Ok(s) => s,
            Err(e) => {
                // Includes the deliberate 404 for delayed games — the poll
                // remains in charge, which is the fail-safe path anyway.
                info!("[spectator] no live feed for game {game_id_owned} ({e}); using the delayed poll");
                return;
            }
        };

        info!("[spectator] live feed open for game {game_id_owned}");

        while !cancel_task.load(Ordering::Relaxed) {
            match subscription.next().await {
                Some(Ok(update)) => {
                    let Some(body) = update.body_str() else { continue };
                    let Ok(ChessMessage::Move(payload)) =
                        serde_json::from_str::<ChessMessage>(body)
                    else {
                        continue;
                    };
                    if let Ok(mut buf) = buffer_task.lock() {
                        buf.push(payload.uci);
                    }
                }
                // A heartbeat miss surfaces as an error; the client reconnects
                // on its own, so keep waiting rather than tearing the feed down.
                Some(Err(e)) => {
                    if matches!(e, braid_chess::braid_http::BraidError::SubscriptionClosed) {
                        info!("[spectator] live feed closed for game {game_id_owned}");
                        break;
                    }
                }
                None => break,
            }
        }
        info!("[spectator] live feed ended for game {game_id_owned}");
    });

    (buffer, cancel)
}

fn drain_live_feed(mut session: ResMut<SpectatorSession>) {
    let Some(feed) = session.live_feed.clone() else {
        return;
    };
    let already_queued = session.applied_move_count + session.pending_moves.len();
    let Ok(buf) = feed.lock() else { return };
    if buf.len() > already_queued {
        let new_moves = buf[already_queued..].to_vec();
        drop(buf);
        session.pending_moves.extend(new_moves);
    }
}

fn tick_spectator_poll(
    mut session: ResMut<SpectatorSession>,
    time: Res<Time>,
    tokio: Res<TokioRuntime>,
) {
    let Some(game_id) = session.game_id.clone() else {
        return;
    };

    // A live subscription is authoritative while it is open — polling too
    // would apply every move twice.
    if session.is_live_subscribed() {
        return;
    }

    session.poll_timer -= time.delta_secs();
    if session.poll_timer > 0.0 {
        return;
    }
    session.poll_timer = SpectatorSession::POLL_INTERVAL;

    let applied = session.applied_move_count;

    let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();
    let game_id_clone = game_id.clone();
    tokio.0.spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            crate::multiplayer::network::vps::get_game_moves_for_spectator(&game_id_clone)
        })
        .await;
        if let Ok(Ok(moves)) = result {
            let _ = tx.send(moves);
        }
    });

    if let Ok(all_moves) = rx.try_recv() {
        if all_moves.len() > applied {
            let new_moves = all_moves[applied..].to_vec();
            session.pending_moves.extend(new_moves);
        }
    }
}

fn dispatch_pending_spectator_moves(
    mut session: ResMut<SpectatorSession>,
    mut move_events: MessageWriter<NetworkMoveEvent>,
    game_mode: Res<GameMode>,
) {
    if *game_mode != GameMode::Spectator {
        return;
    }
    if let Some(uci) = session.pending_moves.first().cloned() {
        if uci.len() >= 4 {
            let from_col = (uci.as_bytes()[0].wrapping_sub(b'a')) as u8;
            let from_row = (uci.as_bytes()[1].wrapping_sub(b'1')) as u8;
            let to_col = (uci.as_bytes()[2].wrapping_sub(b'a')) as u8;
            let to_row = (uci.as_bytes()[3].wrapping_sub(b'1')) as u8;
            let promotion = uci.chars().nth(4).filter(|c| "qrbn".contains(*c));

            move_events.write(NetworkMoveEvent {
                from: (from_col, from_row),
                to: (to_col, to_row),
                promotion,
                expected_fen: None,
                dedup_version: None,
            });
            session.pending_moves.remove(0);
            session.applied_move_count += 1;
        } else {
            session.pending_moves.remove(0);
        }
    }
}

#[cfg(feature = "solana")]
pub fn apply_braid_resync_to_spectator(
    mut rollup_events: MessageReader<crate::multiplayer::rollup::manager::RollupEvent>,
    mut move_events: MessageWriter<NetworkMoveEvent>,
    game_mode: Res<GameMode>,
    mut session: ResMut<SpectatorSession>,
) {
    if *game_mode != GameMode::Spectator {
        return;
    }
    // Never apply live gossip moves for a delayed broadcast (or before the
    // delay is known) — those games are HTTP-delayed-feed only.
    if session.delayed || !session.delay_checked {
        rollup_events.clear();
        return;
    }
    for ev in rollup_events.read() {
        if let crate::multiplayer::rollup::manager::RollupEvent::ResyncedMove {
            move_uci,
            next_fen,
            ..
        } = ev
        {
            let uci = move_uci;
            if uci.len() >= 4 {
                let from_col = (uci.as_bytes()[0].wrapping_sub(b'a')) as u8;
                let from_row = (uci.as_bytes()[1].wrapping_sub(b'1')) as u8;
                let to_col = (uci.as_bytes()[2].wrapping_sub(b'a')) as u8;
                let to_row = (uci.as_bytes()[3].wrapping_sub(b'1')) as u8;
                let promotion = uci.chars().nth(4).filter(|c| "qrbn".contains(*c));

                move_events.write(NetworkMoveEvent {
                    from: (from_col, from_row),
                    to: (to_col, to_row),
                    promotion,
                    expected_fen: Some(next_fen.clone()),
                    dedup_version: None,
                });
                // Advance so the VPS poll (`tick_spectator_poll`) doesn't
                // re-fetch and re-queue moves already applied via gossip
                // resync — previously this counter was only ever read here,
                // never advanced, so it stayed flat across gossip-applied
                // moves (redundant re-fetch/re-queue traffic on every poll
                // cycle, not board corruption — see docs/PRE_MAINNET_E2E_PLAN.md §1.7).
                session.applied_move_count += 1;
            }
        }
    }
}

#[cfg(feature = "solana")]
fn tick_spectator_clock(
    mut clock: ResMut<SpectatorClockState>,
    mut rollup_events: MessageReader<crate::multiplayer::rollup::manager::RollupEvent>,
    game_mode: Res<GameMode>,
    time: Res<Time>,
) {
    if *game_mode != GameMode::Spectator {
        return;
    }

    // Apply any incoming clock snapshots first.
    for ev in rollup_events.read() {
        if let crate::multiplayer::rollup::manager::RollupEvent::SnapshotReceived { .. } = ev {
            // SnapshotReceived carries move history — clock is implicit from move count.
            // A dedicated ClockState message will arrive separately via the publisher.
        }
    }

    // Tick active player's clock down between broadcasts.
    let elapsed_ms = (time.delta_secs_f64() * 1000.0) as u64;
    if clock.last_update_secs > 0.0 {
        if clock.white_to_move {
            clock.white_ms = clock.white_ms.saturating_sub(elapsed_ms);
        } else {
            clock.black_ms = clock.black_ms.saturating_sub(elapsed_ms);
        }
    }
    clock.last_update_secs = time.elapsed_secs_f64();
}

fn toggle_clock_side_on_move(
    mut move_events: MessageReader<NetworkMoveEvent>,
    mut clock: ResMut<SpectatorClockState>,
    game_mode: Res<GameMode>,
) {
    if *game_mode != GameMode::Spectator {
        move_events.clear();
        return;
    }
    for _ in move_events.read() {
        clock.white_to_move = !clock.white_to_move;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_spectate_link() {
        assert_eq!(
            parse_spectate_link("xfchess://spectate/12345"),
            Some("12345".to_string())
        );
    }

    #[test]
    fn rejects_wrong_scheme_and_empty_id() {
        assert_eq!(parse_spectate_link("https://spectate/12345"), None);
        assert_eq!(parse_spectate_link("xfchess://spectate/"), None);
        assert_eq!(parse_spectate_link("garbage"), None);
    }

    #[test]
    fn spectate_link_round_trips() {
        let link = make_spectate_link("777");
        assert_eq!(parse_spectate_link(&link), Some("777".to_string()));
    }

    #[test]
    fn delay_decision_gates_live_gossip() {
        // 0s delay → live game → gossip allowed.
        assert!(!feed_is_delayed(0));
        // Any positive delay → HTTP-only, no live gossip (ghosting defense).
        assert!(feed_is_delayed(1));
        assert!(feed_is_delayed(900));
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;

    fn entry(id: &str) -> SpectatorPlaylistEntry {
        SpectatorPlaylistEntry {
            game_id: id.to_string(),
            white: format!("w{id}"),
            black: format!("b{id}"),
            round: 0,
        }
    }

    #[test]
    fn switching_games_clears_the_previous_board_state() {
        let mut s = SpectatorSession::default();
        s.playlist = vec![entry("1"), entry("2")];
        s.begin_session("1".into(), Some(42));

        s.applied_move_count = 17;
        s.pending_moves.push("e2e4".into());
        s.live_feed = Some(Arc::new(Mutex::new(vec!["e2e4".into()])));
        s.live_subscribed = true;

        s.begin_session("2".into(), Some(42));

        assert_eq!(s.applied_move_count, 0);
        assert!(s.pending_moves.is_empty());
        assert!(s.live_feed.is_none());
        assert!(!s.live_subscribed);
        assert_eq!(s.game_id.as_deref(), Some("2"));
        // The tournament context and playlist survive a switch — that is what
        // makes Next/Prev keep working after hopping.
        assert_eq!(s.tournament_id, Some(42));
        assert_eq!(s.playlist.len(), 2);
        assert_eq!(s.playlist_index, 1);
    }

    #[test]
    fn a_switch_starts_delayed_until_rechecked() {
        let mut s = SpectatorSession::default();
        s.begin_session("1".into(), None);
        s.delayed = false;
        s.delay_checked = true;

        s.begin_session("2".into(), None);

        assert!(
            s.delayed,
            "must not inherit the previous game's live status"
        );
        assert!(!s.delay_checked);
    }

    #[test]
    fn siblings_wrap_and_need_more_than_one_game() {
        let mut s = SpectatorSession::default();
        s.begin_session("1".into(), None);
        assert!(s.sibling(1).is_none(), "nothing to hop to");

        s.playlist = vec![entry("1"), entry("2"), entry("3")];
        s.begin_session("1".into(), None);
        assert_eq!(s.sibling(1).map(|e| e.game_id.as_str()), Some("2"));
        assert_eq!(s.sibling(-1).map(|e| e.game_id.as_str()), Some("3"));

        s.begin_session("3".into(), None);
        assert_eq!(s.sibling(1).map(|e| e.game_id.as_str()), Some("1"));
    }

    #[test]
    fn leaving_drops_the_tournament_context() {
        let mut s = SpectatorSession::default();
        s.playlist = vec![entry("1"), entry("2")];
        s.begin_session("1".into(), Some(7));

        s.leave();

        assert!(s.game_id.is_none());
        assert!(s.tournament_id.is_none());
        assert!(s.playlist.is_empty());
        assert_eq!(s.playlist_index, 0);
    }
}
