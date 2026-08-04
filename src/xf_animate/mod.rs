//! XF Animate — mini chess showcase rendered inside the LEARN box on the main menu.
//!
//! The plugin is strictly scoped to [`GameState::MainMenu`]. All spawned entities
//! carry `DespawnOnExit(MainMenu)` and every system is gated behind
//! `run_if(in_state(MainMenu))` so nothing from this module runs, allocates, or
//! ticks while the actual game is being played.

use bevy::prelude::*;

use crate::core::GameState;

pub mod animation;
pub mod board;
pub mod games;
pub mod pieces;
pub mod sequence;
pub mod viewport;

pub use sequence::SequencePlayback;
pub use viewport::LearnViewportRect;

/// Bevy plugin for the menu-only mini showcase.
pub struct XfAnimatePlugin;

impl Plugin for XfAnimatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LearnViewportRect>()
            .init_resource::<SequencePlayback>()
            .add_systems(
                OnEnter(GameState::MainMenu),
                (
                    viewport::spawn_mini_camera,
                    board::spawn_mini_board,
                    board::spawn_mini_lights,
                    pieces::spawn_mini_pieces,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    viewport::sync_learn_viewport,
                    sequence::run_sequence,
                    sequence::restart_when_complete,
                    animation::animate_moves,
                    animation::animate_captures,
                    animation::idle_float,
                )
                    .run_if(in_state(GameState::MainMenu)),
            );
    }
}
