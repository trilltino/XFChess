//! Chess game plugin
//!
//! This plugin registers all game systems and resources.
//! Systems are organized with run conditions to optimize performance.

use bevy::prelude::*;
use crate::core::GameState;
use super::resources::*;
use super::systems::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        // Register resources
        app.init_resource::<CurrentTurn>()
            .init_resource::<CurrentGamePhase>()
            .init_resource::<Selection>()
            .init_resource::<MoveHistory>()
            .init_resource::<GameTimer>();

        // Register systems with run conditions
        // Only run game systems when in Multiplayer state
        app.add_systems(
            Update,
            (
                // Handle input and selection first
                handle_piece_selection,
                clear_selection_on_empty_click,

                // Then handle moves
                move_piece,

                // Update game state
                update_game_phase,
                update_game_timer,

                // Visual updates last
                highlight_possible_moves,
                animate_piece_movement,
            )
                .run_if(in_state(GameState::Multiplayer)),
        );
    }
}
