//! Game over screen plugin
//!
//! Displayed when a game concludes (checkmate, stalemate, or resignation).
//! The board/pieces/camera are left exactly where they were when the game
//! ended (see `cleanup_in_game` in `core/state_lifecycle.rs`) — this plugin
//! records stats and clears the pulsing check-highlight light so it doesn't
//! persist into the frozen board. The popup plugin covers the frozen scene
//! with an opaque background (see `ui::menus::game_over_popup`).
//! For Solana wager games, displays detailed fee breakdown from smart contract.

use crate::core::GameState;
use crate::game::resources::{GameOverState, MoveHistory};
use crate::rendering::effects::CheckHighlightLight;
use crate::ui::menus::game_over_popup::GameOverPopupPlugin;
use bevy::prelude::*;

/// Plugin for game over screen: stats recording + popup UI
pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        // Add the popup plugin
        app.add_plugins(GameOverPopupPlugin);

        app.add_systems(
            OnEnter(GameState::GameOver),
            (clear_check_highlight, record_game_stats),
        );
    }
}

/// The check-highlight light is normally cleared by `update_check_highlight_system`
/// (InGame only) or `DespawnOnExit(InGame)`, but the latter is deliberately
/// deferred until GameOver is exited (see `cleanup_in_game`) so the frozen
/// board is visible — so a light left on at the moment of checkmate would
/// otherwise persist through the whole GameOver screen.
fn clear_check_highlight(
    mut commands: Commands,
    lights: Query<Entity, With<CheckHighlightLight>>,
) {
    for entity in lights.iter() {
        commands.entity(entity).despawn();
    }
}

/// Record game statistics when entering game over state
fn record_game_stats(
    game_over: Res<GameOverState>,
    move_history: Res<MoveHistory>,
    mut stats: ResMut<crate::core::GameStatistics>,
) {
    let winner = game_over.winner();
    let moves = move_history.len() as u32;

    stats.record_game(winner, moves);
    info!(
        "[GAME_OVER] Game statistics recorded: winner={:?}, moves={}",
        winner, moves
    );
}
