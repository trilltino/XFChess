use crate::core::GameState;
use crate::game::resources::{GameOverState, MoveHistory};
use crate::rendering::effects::CheckHighlightLight;
use crate::ui::menus::game_over_popup::GameOverPopupPlugin;
use bevy::prelude::*;

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
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

    // Support-bundle marker: [GAME-END] closes the [GAME-START] line above;
    // online games carry their numeric game_id on both.
    let game_id = crate::multiplayer::network::game_id_store::get();
    if game_id != 0 {
        info!(
            "[GAME-END] game_id={} result={:?} winner={:?} moves={}",
            game_id, *game_over, winner, moves
        );
    } else {
        info!(
            "[GAME-END] result={:?} winner={:?} moves={}",
            *game_over, winner, moves
        );
    }
}
