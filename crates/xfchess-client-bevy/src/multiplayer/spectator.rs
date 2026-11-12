use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::state::{GameMode, GameState};
use crate::engine::board_state::ChessEngine;
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
    poll_result: Option<
        crossbeam_channel::Receiver<crate::multiplayer::network::vps::game::SpectatorMovesResponse>,
    >,
    in_flight: Option<(String, String)>,
}

impl SpectatorSession {
    pub const POLL_INTERVAL: f32 = 2.0;

    pub fn is_live_subscribed(&self) -> bool {
        self.live_feed.is_some()
            && self
                .live_cancel
                .as_ref()
                .is_some_and(|flag| !flag.load(Ordering::Relaxed))
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
        self.poll_result = None;
        self.in_flight = None;
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
            .add_message::<crate::game::events::NetworkMoveResult>()
            .add_systems(
                Update,
                (
                    handle_spectate_link,
                    resolve_spectator_delay,
                    tick_spectator_poll,
                    drain_live_feed,
                    dispatch_pending_spectator_moves,
                    toggle_clock_side_on_move,
                )
                    .chain()
                    .in_set(crate::core::schedule::ReceiveSet::Feeds),
            );
        app.add_systems(
            Update,
            confirm_spectator_move.in_set(crate::core::schedule::SendSet::Session),
        );
        #[cfg(feature = "solana")]
        app.add_systems(
            Update,
            (apply_braid_resync_to_spectator, tick_spectator_clock)
                .chain()
                .after(toggle_clock_side_on_move)
                .in_set(crate::core::schedule::ReceiveSet::Feeds),
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
        // Start in delayed HTTP-only mode until lookup confirms zero delay. Set
        // the playlist before begin_session so its index uses the right game list.
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
        struct Finished(Arc<AtomicBool>);
        impl Drop for Finished { fn drop(&mut self) { self.0.store(true, Ordering::Relaxed); } }
        let _finished = Finished(cancel_task.clone());
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

        let request = BraidRequest::new()
            .subscribe()
            .with_heartbeat(20)
            .with_retry(braid_chess::braid_http::client::RetryConfig::default().with_max_retries(5));
        let cancelled = async {
            while !cancel_task.load(Ordering::Relaxed) { tokio::time::sleep(std::time::Duration::from_millis(100)).await; }
        };
        tokio::pin!(cancelled);
        let opened = tokio::select! { _ = &mut cancelled => return, opened = client.subscribe_reliable(&url, request) => opened };
        let mut subscription = match opened {
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
            let update = tokio::select! { _ = &mut cancelled => break, update = subscription.next() => update };
            match update {
                Some(Ok(update)) => {
                    // The feed is read-only: an update is consumed once it is buffered or skipped.
                    let mover = update.body_str().and_then(|body| serde_json::from_str::<ChessMessage>(body).ok());
                    if let Some(ChessMessage::Move(payload)) = mover {
                        if let Ok(mut buf) = buffer_task.lock() {
                            buf.push(payload.uci);
                        }
                    }
                    if subscription.acknowledge(&update).await.is_err() {
                        break;
                    }
                }
                Some(Err(e)) => {
                    info!("[spectator] live feed ended for game {game_id_owned}: {e}; using polling");
                    break;
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
    let already_queued = session.applied_move_count
        + session.pending_moves.len()
        + usize::from(session.in_flight.is_some());
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
    mut spectate_events: MessageWriter<SpectateViaLinkEvent>,
    match_info: Option<Res<SpectatorMatchInfo>>,
) {
    let Some(game_id) = session.game_id.clone() else {
        return;
    };

    // A live subscription is authoritative while it is open — polling too
    // would apply every move twice.
    if session.is_live_subscribed() {
        return;
    }

    if let Some(rx) = session.poll_result.as_ref() {
        match rx.try_recv() {
            Ok(response) => {
                if response.reset {
                    spectate_events.write(SpectateViaLinkEvent {
                        game_id,
                        tournament_id: session.tournament_id,
                        details: match_info.as_ref().map(|m| m.0.clone()),
                        playlist: session.playlist.clone(),
                    });
                    session.end_session();
                    return;
                }
                let queued = session.applied_move_count
                    + session.pending_moves.len()
                    + usize::from(session.in_flight.is_some());
                session.pending_moves.extend(
                    response
                        .moves
                        .into_iter()
                        .filter(|m| m.move_number > queued)
                        .map(|m| m.move_uci),
                );
                session.poll_result = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => return,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                session.poll_result = None;
            }
        }
    }

    session.poll_timer -= time.delta_secs();
    if session.poll_timer > 0.0 {
        return;
    }
    session.poll_timer = SpectatorSession::POLL_INTERVAL;

    let after_move = session.applied_move_count
        + session.pending_moves.len()
        + usize::from(session.in_flight.is_some());
    let (tx, rx) = crossbeam_channel::bounded(1);
    session.poll_result = Some(rx);
    let game_id_clone = game_id;
    tokio.0.spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            crate::multiplayer::network::vps::get_game_moves_for_spectator(
                &game_id_clone,
                after_move,
            )
        })
        .await;
        if let Ok(Ok(moves)) = result {
            let _ = tx.send(moves);
        }
    });
}

fn dispatch_pending_spectator_moves(
    mut session: ResMut<SpectatorSession>,
    mut move_events: MessageWriter<NetworkMoveEvent>,
    game_mode: Res<GameMode>,
) {
    if *game_mode != GameMode::Spectator {
        return;
    }
    if session.in_flight.is_some() {
        return;
    }
    if let Some(uci) = session.pending_moves.first().cloned() {
        if let Some((from, to, promotion)) = ChessEngine::parse_uci_move(&uci) {
            let version = format!(
                "spectator:{}/{}",
                session.game_id.as_deref().unwrap_or_default(),
                session.applied_move_count + 1
            );
            move_events.write(NetworkMoveEvent {
                from,
                to,
                promotion,
                expected_fen: None,
                dedup_version: Some(version.clone()),
            });
            session.pending_moves.remove(0);
            session.in_flight = Some((version, uci));
        } else {
            session.pending_moves.remove(0);
        }
    }
}

fn confirm_spectator_move(
    mut session: ResMut<SpectatorSession>,
    mut results: MessageReader<crate::game::events::NetworkMoveResult>,
) {
    for result in results.read() {
        if let Some((_, uci)) = session
            .in_flight
            .take_if(|(version, _)| Some(&*version) == result.version.as_ref())
        {
            if result.fen.is_some() {
                session.applied_move_count += 1;
            } else {
                session.pending_moves.insert(0, uci);
            }
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
    if session.is_live_subscribed() {
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
            if let Some((from, to, promotion)) = ChessEngine::parse_uci_move(move_uci) {
                move_events.write(NetworkMoveEvent {
                    from,
                    to,
                    promotion,
                    expected_fen: Some(next_fen.clone()),
                    dedup_version: None,
                });
                // Advance the applied move count after gossip resync so VPS polling does
                // not fetch and queue those moves again.
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
    fn exhausted_live_feed_allows_polling_and_keeps_its_response_channel(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new();
        app.add_message::<SpectateViaLinkEvent>();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(TokioRuntime(tokio::runtime::Runtime::new()?));
        let (tx, rx) = crossbeam_channel::bounded(1);
        let mut session = SpectatorSession::default();
        session.begin_session("1".into(), None);
        session.applied_move_count = 1;
        session.pending_moves.push("e7e5".into());
        session.poll_timer = 30.;
        session.live_feed = Some(Arc::new(Mutex::new(vec![])));
        session.live_cancel = Some(Arc::new(AtomicBool::new(true)));
        session.poll_result = Some(rx);
        assert!(!session.is_live_subscribed());
        app.insert_resource(session);
        app.add_systems(Update, tick_spectator_poll);
        app.update();
        assert!(app
            .world()
            .resource::<SpectatorSession>()
            .poll_result
            .is_some());
        tx.send(
            crate::multiplayer::network::vps::game::SpectatorMovesResponse {
                reset: false,
                moves: ["e2e4", "e7e5", "g1f3"]
                    .into_iter()
                    .enumerate()
                    .map(
                        |(i, uci)| crate::multiplayer::network::vps::game::SpectatorMoveEntry {
                            move_number: i + 1,
                            move_uci: uci.into(),
                        },
                    )
                    .collect(),
            },
        )?;
        app.update();
        let session = app.world().resource::<SpectatorSession>();
        assert_eq!(session.pending_moves, ["e7e5", "g1f3"]);
        assert!(session.poll_result.is_none());

        Ok(())
    }

    #[test]
    fn invalid_cursor_restarts_the_session_before_replaying_moves(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new();
        app.add_message::<SpectateViaLinkEvent>();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(TokioRuntime(tokio::runtime::Runtime::new()?));
        let (tx, rx) = crossbeam_channel::bounded(1);
        let mut session = SpectatorSession::default();
        session.begin_session("1".into(), Some(7));
        session.applied_move_count = 20;
        session.pending_moves.push("e2e4".into());
        session.poll_result = Some(rx);
        app.insert_resource(session);
        tx.send(
            crate::multiplayer::network::vps::game::SpectatorMovesResponse {
                reset: true,
                moves: vec![],
            },
        )?;
        app.add_systems(Update, tick_spectator_poll);
        app.update();
        let session = app.world().resource::<SpectatorSession>();
        assert!(session.game_id.is_none());
        assert_eq!(session.applied_move_count, 0);
        assert!(session.pending_moves.is_empty());
        let events = app.world().resource::<Messages<SpectateViaLinkEvent>>();
        let mut cursor = events.get_cursor();
        let restart = cursor.read(events).next().ok_or("missing test value")?;
        assert_eq!(restart.game_id, "1");
        assert_eq!(restart.tournament_id, Some(7));

        Ok(())
    }

    #[test]
    fn spectator_counts_only_engine_accepted_moves() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new();
        app.insert_resource(GameMode::Spectator);
        let mut session = SpectatorSession::default();
        session.begin_session("1".into(), None);
        session.pending_moves.push("e2e4".into());
        app.insert_resource(session);
        app.add_message::<NetworkMoveEvent>();
        app.add_message::<crate::game::events::NetworkMoveResult>();
        app.add_systems(
            Update,
            (confirm_spectator_move, dispatch_pending_spectator_moves).chain(),
        );
        app.update();
        assert_eq!(
            app.world()
                .resource::<SpectatorSession>()
                .applied_move_count,
            0
        );
        let version = app
            .world()
            .resource::<SpectatorSession>()
            .in_flight
            .as_ref()
            .ok_or("move was not in flight")?
            .0
            .clone();
        app.world_mut()
            .write_message(crate::game::events::NetworkMoveResult {
                version: Some(version.clone()),
                fen: None,
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<SpectatorSession>()
                .applied_move_count,
            0
        );
        app.world_mut()
            .write_message(crate::game::events::NetworkMoveResult {
                version: Some(version),
                fen: Some("accepted".into()),
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<SpectatorSession>()
                .applied_move_count,
            1
        );
        assert!(app
            .world()
            .resource::<SpectatorSession>()
            .in_flight
            .is_none());

        Ok(())
    }

    #[test]
    fn switching_games_clears_the_previous_board_state() {
        let mut s = SpectatorSession {
            playlist: vec![entry("1"), entry("2")],
            ..Default::default()
        };
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
        let mut s = SpectatorSession {
            playlist: vec![entry("1"), entry("2")],
            ..Default::default()
        };
        s.begin_session("1".into(), Some(7));

        s.leave();

        assert!(s.game_id.is_none());
        assert!(s.tournament_id.is_none());
        assert!(s.playlist.is_empty());
        assert_eq!(s.playlist_index, 0);
    }
}
