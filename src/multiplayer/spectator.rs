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

/// Bevy plugin for spectator mode.
pub struct SpectatorPlugin;

impl Plugin for SpectatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorSession>()
            .add_message::<SpectateViaLinkEvent>()
            .add_systems(Update, (
                handle_spectate_link,
                tick_spectator_poll,
                dispatch_pending_spectator_moves,
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
) {
    for ev in events.read() {
        info!("[spectator] Starting spectate for game {}", ev.game_id);
        session.game_id = Some(ev.game_id.clone());
        session.applied_move_count = 0;
        session.poll_timer = 0.0;
        session.pending_moves.clear();
        *game_mode = GameMode::Spectator;
        next_state.set(GameState::InGame);
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

    // Run blocking HTTP in spawn_blocking so we don't stall the render thread.
    // We can't await here so we write directly via a channel-less approach:
    // store results straight into the resource after `block_in_place`.
    // Since this is a regular Bevy system (sync context) we use spawn_blocking
    // and rely on the result arriving next frame via the pending_moves vec.
    //
    // Limitation: the task result is not available this frame; it arrives next
    // frame when tokio fires the oneshot.  We use a simple approach: spawn +
    // read a shared Arc<Mutex<>> flag.  For simplicity, poll synchronously
    // inside IoTaskPool via `block_in_place` (only safe when called from an
    // async context, so we use `std::thread::spawn` as a fallback).

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

    // Collect any result that arrived (possibly from previous spawn).
    // We spin the channel without blocking to avoid stalling the game thread.
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
            // Malformed UCI — skip it.
            session.pending_moves.remove(0);
        }
    }
}
