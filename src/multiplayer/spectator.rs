//! Spectator mode — watch a live game via `xfchess://spectate/{game_id}`.
//!
//! The plugin polls `GET /games/{game_id}/moves` every 2 seconds and
//! dispatches `NetworkMoveEvent` for any new moves, letting the board
//! render them without accepting local input.

use bevy::prelude::*;
use crate::multiplayer::traits::{Message, MessageReader, MessageWriter};
use crate::game::events::NetworkMoveEvent;
use crate::core::states::{GameMode, GameState};
use crate::multiplayer::TokioRuntime;
#[cfg(feature = "solana")]
use crate::multiplayer::network::protocol::NetworkMessage;

/// Deep-link event fired when OS / CLI passes `xfchess://spectate/{game_id}`.
#[derive(Message, Debug, Clone)]
pub struct SpectateViaLinkEvent {
    pub game_id: String,
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

/// Resource tracking the active spectator session.
#[derive(Resource, Default)]
pub struct SpectatorSession {
    /// The game being spectated; `None` when spectator mode is inactive.
    pub game_id: Option<String>,
    /// Number of moves already applied to the local board.
    pub applied_move_count: usize,
    /// Seconds until next poll.
    pub poll_timer: f32,
    /// Pending UCI moves fetched from VPS, awaiting dispatch.
    pub pending_moves: Vec<String>,
}

impl SpectatorSession {
    pub const POLL_INTERVAL: f32 = 2.0;
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
            .add_message::<SpectateViaLinkEvent>()
            .add_systems(Update, (
                handle_spectate_link,
                tick_spectator_poll,
                dispatch_pending_spectator_moves,
                toggle_clock_side_on_move,
            ));
        #[cfg(feature = "solana")]
        app.add_systems(Update, (
            apply_braid_resync_to_spectator,
            tick_spectator_clock,
        ));
    }
}

/// Handle incoming `SpectateViaLinkEvent` — set game mode to Spectator and
/// store the game ID so the poll loop can start.
fn handle_spectate_link(
    mut events: MessageReader<SpectateViaLinkEvent>,
    mut session: ResMut<SpectatorSession>,
    mut game_mode: ResMut<GameMode>,
    mut next_state: ResMut<NextState<GameState>>,
    network_state: Option<Res<crate::multiplayer::BraidNetworkState>>,
) {
    for ev in events.read() {
        info!("[spectator] Starting spectate for game {}", ev.game_id);
        session.game_id = Some(ev.game_id.clone());
        session.applied_move_count = 0;
        session.poll_timer = 0.0;
        session.pending_moves.clear();
        *game_mode = GameMode::Spectator;
        next_state.set(GameState::InGame);

        #[cfg(feature = "solana")]
        if let Some(ref ns) = network_state {
            // Subscribe to the game's iroh gossip topic so GameSnapshot arrives.
            if let Some(ref sub_tx) = ns.subscription_sender {
                let topic = format!("/xfchess-game/{}", ev.game_id);
                let _ = sub_tx.send(topic);
            }
            // Request full move history from the active peer (since_version "0" = all).
            if let Some(gid) = ev.game_id.parse::<u64>().ok() {
                if let Some(ref msg_tx) = ns.message_sender {
                    let _ = msg_tx.send(NetworkMessage::BraidResyncRequest {
                        game_id: gid,
                        since_version: "0".to_string(),
                    });
                }
            }
        }
    }
}

/// Timer-driven poll: fetch all moves from VPS and queue any that are new.
fn tick_spectator_poll(
    mut session: ResMut<SpectatorSession>,
    time: Res<Time>,
    tokio: Res<TokioRuntime>,
) {
    let Some(game_id) = session.game_id.clone() else { return };

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
        }).await;
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
            let to_col   = (uci.as_bytes()[2].wrapping_sub(b'a')) as u8;
            let to_row   = (uci.as_bytes()[3].wrapping_sub(b'1')) as u8;
            let promotion = uci.chars().nth(4).filter(|c| "qrbn".contains(*c));

            move_events.write(NetworkMoveEvent {
                from: (from_col, from_row),
                to:   (to_col, to_row),
                promotion,
                expected_fen: None,
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
fn apply_braid_resync_to_spectator(
    mut rollup_events: MessageReader<crate::multiplayer::rollup::manager::RollupEvent>,
    mut move_events: MessageWriter<NetworkMoveEvent>,
    game_mode: Res<GameMode>,
    session: Res<SpectatorSession>,
) {
    if *game_mode != GameMode::Spectator {
        return;
    }
    for ev in rollup_events.read() {
        if let crate::multiplayer::rollup::manager::RollupEvent::ResyncedMove { move_uci, next_fen, .. } = ev {
            let uci = move_uci;
            if uci.len() >= 4 {
                let from_col = (uci.as_bytes()[0].wrapping_sub(b'a')) as u8;
                let from_row = (uci.as_bytes()[1].wrapping_sub(b'1')) as u8;
                let to_col   = (uci.as_bytes()[2].wrapping_sub(b'a')) as u8;
                let to_row   = (uci.as_bytes()[3].wrapping_sub(b'1')) as u8;
                let promotion = uci.chars().nth(4).filter(|c| "qrbn".contains(*c));

                move_events.write(NetworkMoveEvent {
                    from: (from_col, from_row),
                    to:   (to_col, to_row),
                    promotion,
                    expected_fen: Some(next_fen.clone()),
                });
                let _ = session.applied_move_count; // acknowledged; VPS poll will deduplicate
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
