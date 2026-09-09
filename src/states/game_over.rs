use crate::core::GameState;
use crate::game::resources::{GameOverState, MoveHistory};
use crate::rendering::effects::CheckHighlightLight;
use crate::ui::menus::game_over_popup::GameOverPopupPlugin;
use bevy::prelude::*;

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

fn clear_check_highlight(mut commands: Commands, lights: Query<Entity, With<CheckHighlightLight>>) {
    for entity in lights.iter() {
        commands.entity(entity).despawn();
    }
}

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
