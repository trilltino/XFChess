//! Spectator mode — watch a game via `xfchess://spectate/{game_id}` or a
//! tournament's Watch button.
//!
//! # Two feeds, one of them instant
//!
//! For a **live** game (broadcast delay 0) the spectator opens a Braid `209`
//! subscription to `/spectate/{game_id}/moves`: it receives the moves already
//! played, then each new move the moment `record_move` records it. There is no
//! catch-up path, because snapshot-then-tail *is* the catch-up.
//!
//! For a **delayed** game the subscription is refused by the backend and the
//! spectator falls back to polling `GET /games/moves/{game_id}` every
//! [`SpectatorSession::POLL_INTERVAL`] seconds, which filters moves newer than
//! the delay horizon. Exactly one of the two drives the board at a time — see
//! [`SpectatorSession::live_feed`].
//!
//! # Broadcast integrity
//!
//! Before opening anything live, the spectator queries the game's broadcast
//! delay. A game with a non-zero delay (tournament/esports) is watched *only*
//! through the delay-gated HTTP feed — neither the Braid subscription nor the
//! live gossip subscription is opened, so the stream can't be used to ghost.
//! The default until the delay is known is "delayed" (fail safe), and the
//! backend independently refuses to publish a delayed game's moves at all
//! (`routes::spectate::is_streamable`). Losing either guard alone does not
//! leak a live board.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::states::{GameMode, GameState};
use crate::game::events::NetworkMoveEvent;
#[cfg(feature = "solana")]
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::traits::{Message, MessageReader, MessageWriter};
use crate::multiplayer::TokioRuntime;
use bevy::prelude::*;

/// Who/what is being watched — carried alongside a spectate request when the
/// UI knows it (tournament lists), absent for bare deep links.
#[derive(Debug, Clone, Default)]
pub struct SpectatorMatchDetails {
    pub tournament_name: Option<String>,
    /// 0-based round index (display as round + 1).
    pub round: Option<u8>,
    /// Display labels (username or truncated pubkey).
    pub white: Option<String>,
    pub black: Option<String>,
}

/// Resource holding the current spectated match's details for the HUD.
#[derive(Resource, Default)]
pub struct SpectatorMatchInfo(pub SpectatorMatchDetails);

/// Deep-link event fired when OS / CLI passes `xfchess://spectate/{game_id}`.
#[derive(Message, Debug, Clone)]
pub struct SpectateViaLinkEvent {
    pub game_id: String,
    /// Match context when known (tournament watch buttons); None for deep links.
    pub details: Option<SpectatorMatchDetails>,
    /// The tournament being watched, when the request came from a tournament
    /// list. Exiting returns here instead of the top-level menu.
    pub tournament_id: Option<u64>,
    /// The other watchable games to offer as next/previous. Empty for deep
    /// links and one-off watch buttons.
    pub playlist: Vec<SpectatorPlaylistEntry>,
}

/// Parse a spectate link.
pub fn parse_spectate_link(url: &str) -> Option<String> {
    url.strip_prefix("xfchess://spectate/")
        .filter(|id| !id.is_empty())
        .map(|id| id.to_string())
}

/// Generate a spectate link for sharing.
pub fn make_spectate_link(game_id: &str) -> String {
    format!("xfchess://spectate/{}", game_id)
}

/// Whether a game with the given broadcast delay must be watched on the
/// delayed HTTP feed only (no live gossip). Pure so the broadcast-integrity
/// decision is unit-testable without the Bevy/iroh stack.
pub fn feed_is_delayed(delay_secs: u64) -> bool {
    delay_secs > 0
}

/// The ordered UCI move list a live subscription maintains, shared with the
/// Bevy world. History first, then each new move as it is published.
pub type LiveFeedBuffer = Arc<Mutex<Vec<String>>>;

/// One watchable game alongside the current one, so a viewer can move between
/// games without going back to the tournament screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectatorPlaylistEntry {
    pub game_id: String,
    pub white: String,
    pub black: String,
    pub round: u8,
}

/// Resource tracking the active spectator session.
#[derive(Resource, Default)]
pub struct SpectatorSession {
    /// The game being spectated; `None` when spectator mode is inactive.
    pub game_id: Option<String>,
    /// The tournament this game belongs to, so exiting returns there rather
    /// than dumping the viewer at the top-level menu.
    pub tournament_id: Option<u64>,
    /// Number of moves already applied to the local board.
    pub applied_move_count: usize,
    /// Seconds until next poll.
    pub poll_timer: f32,
    /// Pending UCI moves fetched from VPS, awaiting dispatch.
    pub pending_moves: Vec<String>,
    /// True while this game must be watched via the delayed HTTP feed only —
    /// no live gossip. Starts true (fail safe) until the delay is confirmed 0.
    pub delayed: bool,
    /// Whether the broadcast-delay lookup has resolved.
    pub delay_checked: bool,
    /// Async slot for the broadcast-delay lookup result (seconds).
    pub delay_result: Option<Arc<Mutex<Option<u64>>>>,
    /// Move list maintained by the Braid `209` subscription, when one is open.
    ///
    /// While this is `Some`, it is the **only** move source — the poll stands
    /// down, so a move can't be applied twice from two feeds.
    pub live_feed: Option<LiveFeedBuffer>,
    /// Set once a subscription has been spawned for the current game, so it is
    /// not spawned again every frame.
    pub live_subscribed: bool,
    /// Cleared to tell the subscription task for a previous game to stop.
    pub live_cancel: Option<Arc<AtomicBool>>,
    /// The other watchable games in this tournament, in board order.
    ///
    /// Captured when spectating starts from a tournament list, so hopping to
    /// the next game needs no round-trip. Survives a game switch; cleared only
    /// on leaving spectator mode.
    pub playlist: Vec<SpectatorPlaylistEntry>,
    /// Index of the current game within [`Self::playlist`].
    pub playlist_index: usize,
}

impl SpectatorSession {
    /// Fallback poll cadence, used only for delayed games and when a live
    /// subscription could not be opened.
    pub const POLL_INTERVAL: f32 = 2.0;

    /// Whether a live Braid subscription is carrying this game's moves.
    pub fn is_live_subscribed(&self) -> bool {
        self.live_feed.is_some()
    }

    /// Tear down the current session: stop any subscription task and clear all
    /// per-game state.
    ///
    /// Called both when leaving spectator mode and when switching to another
    /// game. Without the reset, board state from the previous game leaks into
    /// the next one — the move indices are per-game.
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

    /// Point the session at `game_id`, keeping the tournament context.
    ///
    /// Fails safe the same way a fresh spectate does: `delayed = true` until
    /// the per-game delay lookup says otherwise.
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

    /// Leave spectating entirely — drops the playlist as well as the session.
    pub fn leave(&mut self) {
        self.end_session();
        self.tournament_id = None;
        self.playlist.clear();
        self.playlist_index = 0;
    }

    /// The game `offset` positions away in the playlist, wrapping around.
    ///
    /// Returns `None` when there is nothing else to watch, which is what
    /// disables the next/previous controls.
    pub fn sibling(&self, offset: isize) -> Option<&SpectatorPlaylistEntry> {
        if self.playlist.len() < 2 {
            return None;
        }
        let len = self.playlist.len() as isize;
        let idx = (self.playlist_index as isize + offset).rem_euclid(len);
        self.playlist.get(idx as usize)
    }
}

/// Clock state for the spectated game, updated via Braid clock broadcasts.
#[derive(Resource, Default)]
pub struct SpectatorClockState {
    pub white_ms: u64,
    pub black_ms: u64,
    /// Whether white is currently on the clock (last move was by black).
    pub white_to_move: bool,
    /// Local time (in seconds) when this state was last updated, for interpolation.
    pub last_update_secs: f64,
}

/// Bevy plugin for spectator mode.
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

/// Handle incoming `SpectateViaLinkEvent` — set game mode to Spectator and
/// store the game ID so the poll loop can start.
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

/// Resolves the broadcast-delay lookup and, only for a live game (delay 0),
/// opens the P2P gossip subscription + resync. A delayed game is watched
/// exclusively through the delay-gated HTTP poll, so its live board can't be
/// pulled over gossip to ghost a stream.
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

/// Open a Braid `209` subscription to a game's spectator feed.
///
/// The stream delivers the moves already played, then each new move the instant
/// `record_move` records it — so a viewer joining at move 20 sees all 20 and
/// then follows live, with no catch-up path and no polling.
///
/// Returns the shared buffer the stream writes into, plus the flag that stops
/// it. A `404` means the backend refuses to stream this game (it has a
/// broadcast delay), and the caller stays on the delayed poll.
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

/// Move anything the live subscription has received onto the dispatch queue.
///
/// The buffer holds the game's whole move list in order, so `applied_move_count`
/// indexes straight into it — the same arithmetic the poll uses, which is what
/// lets either source drive the board without the other knowing.
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

/// Timer-driven poll: fetch all moves from VPS and queue any that are new.
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

/// Dispatch one pending move per frame as a `NetworkMoveEvent`.
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

/// Apply `RollupEvent::ResyncedMove` events to the spectator board — this is the
/// fast path (arrives via gossip) versus the 2-second VPS poll.
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

/// Update `SpectatorClockState` from incoming Braid clock messages and tick
/// the active player's clock down locally between broadcasts.
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

/// Toggle `SpectatorClockState::white_to_move` each time a move is applied to the
/// spectator board, so the local interpolation always ticks the right player's clock.
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

    /// Switching games must not carry the previous game's move cursor over —
    /// move indices are per-game, so a leaked `applied_move_count` silently
    /// skips the start of the next game.
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

    /// A switch must fail safe the same way a fresh spectate does: delayed
    /// until the new game's delay lookup resolves.
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

    /// Leaving drops the playlist too, so a later deep link doesn't inherit
    /// Next/Prev buttons pointing at an unrelated tournament.
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
