//! Replaying a finished tournament game.
//!
//! The Finished tab's Replay button can't load a game inline — the PGN has to
//! be fetched, and the tournament browser runs inside an egui closure. So the
//! button parks a request here, a background task fetches, and
//! [`poll_pgn_replay_fetch`] hands the result to the *same* replay player the
//! PGN-paste modal drives ([`ParsedPgnGameResource`] + `CoreGameMode::PgnReplay`).
//!
//! A tournament game therefore replays through exactly one code path, shared
//! with every other replay in the app.

use bevy::prelude::*;
use tracing::{info, warn};

use crate::core::states::GameState;
use crate::game::replay::ParsedPgnGameResource;

/// An in-flight PGN fetch for a game the viewer asked to replay.
pub struct PendingPgnReplay {
    pub rx: crossbeam_channel::Receiver<Result<String, String>>,
    /// Display names, used for the PGN tags when the fetched PGN has none.
    pub white: String,
    pub black: String,
}

/// Holds the single in-flight replay request, if any.
#[derive(Resource, Default)]
pub struct PgnReplayFetch {
    pub pending: Option<PendingPgnReplay>,
    /// Surfaced in the browser when a replay could not be loaded.
    pub error: Option<String>,
}

/// Completes a replay request: parse the PGN and enter the replay player.
pub fn poll_pgn_replay_fetch(
    mut fetch: ResMut<PgnReplayFetch>,
    mut commands: Commands,
    mut core_mode: ResMut<crate::core::GameMode>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Some(pending) = fetch.pending.as_ref() else {
        return;
    };

    let received = match pending.rx.try_recv() {
        Ok(v) => v,
        Err(crossbeam_channel::TryRecvError::Empty) => return,
        Err(_) => {
            fetch.pending = None;
            fetch.error = Some("replay fetch cancelled".to_string());
            return;
        }
    };

    let (white, black) = (pending.white.clone(), pending.black.clone());
    fetch.pending = None;

    let pgn_text = match received {
        Ok(text) => text,
        Err(e) => {
            warn!("[replay] could not fetch PGN: {e}");
            fetch.error = Some(e);
            return;
        }
    };

    match nimzovich_engine::parse_pgn(&pgn_text) {
        Ok(mut pgn) => {
            info!("[replay] loaded tournament game: {} moves", pgn.moves.len());
            // The backend assembles PGN from stored SAN and doesn't always
            // carry player tags; fill them so the replay header names the
            // players the viewer just clicked on.
            pgn.tags
                .entry("White".to_string())
                .or_insert_with(|| white.clone());
            pgn.tags
                .entry("Black".to_string())
                .or_insert_with(|| black.clone());
            commands.insert_resource(ParsedPgnGameResource {
                inner: pgn,
                show_eval_graph: false,
                puzzle_mode: false,
                puzzle_revealed: false,
            });
            *core_mode = crate::core::GameMode::PgnReplay;
            next_state.set(GameState::InGame);
            fetch.error = None;
        }
        Err(e) => {
            warn!("[replay] could not parse PGN: {e:?}");
            fetch.error = Some(format!("could not parse this game's PGN: {e:?}"));
        }
    }
}
