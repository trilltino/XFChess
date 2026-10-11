//! Chess puzzles, checked by the server one move at a time.
//!
//! The client only draws the board and collects moves: the backend
//! (`backend/src/signing/routes/puzzle.rs`) holds each solution and reveals the
//! opponent's reply only after a correct move, so solutions never ship in the
//! client and ratings can't be farmed by reading responses ahead.
//!
//! - [`api`] — the backend puzzle endpoints (blocking; run on background threads).
//! - [`session`] — the play loop and the [`StartPuzzle`] message that begins it.
//! - [`hub`] — the main-menu panel for choosing a puzzle kind, theme and difficulty.
//! - [`hud`] — the in-game card with progress, hint, result and "Next".
//! - [`uci`] — UCI move strings to and from board coordinates.
//!
//! Puzzles come from the imported Lichess database (rated training, the daily
//! puzzle, SOL bounties) or, when the player has linked Lichess, straight from
//! their Lichess account, with results synced back.

pub mod api;
mod hub;
mod hud;
mod session;
pub mod uci;

pub use hub::{show_hub, HubAction, PuzzleHub};
pub use session::{PuzzleBoard, PuzzlePhase, PuzzleSession, StartPuzzle};

use bevy::prelude::*;

use crate::core::states::{GameMode, GameState, InGameplay};

pub struct PuzzlePlugin;

impl Plugin for PuzzlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PuzzleSession>()
            .init_resource::<PuzzleBoard>()
            .init_resource::<PuzzleHub>()
            .add_message::<StartPuzzle>()
            .add_systems(
                Update,
                (
                    session::start_puzzle,
                    session::poll_loading,
                    hub::poll_themes,
                )
                    .chain()
                    .in_set(crate::core::schedule::SessionSet::Puzzle),
            )
            .add_systems(
                Update,
                (
                    session::set_up_board,
                    session::submit_player_move,
                    session::poll_move,
                    session::poll_hint,
                )
                    .chain()
                    .in_set(crate::core::schedule::MoveSource::Puzzle)
                    .run_if(resource_equals(GameMode::Puzzle)),
            )
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                hud::puzzle_hud.run_if(
                    in_state(GameState::InGame).and_then(resource_equals(GameMode::Puzzle)),
                ),
            )
            .add_systems(OnExit(InGameplay), session::end_on_exit);
    }
}
