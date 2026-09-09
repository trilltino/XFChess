use crate::core::{GameMode, GameState};
use crate::game::events::FlagTimeoutEvent;
use crate::game::resources::{FirstMoveDeadline, GameOverState, MoveHistory};
use bevy::prelude::*;

pub fn start_first_move_deadline(
    mut deadline: ResMut<FirstMoveDeadline>,
    game_mode: Res<GameMode>,
) {
    if matches!(
        *game_mode,
        GameMode::OnlineMultiplayer | GameMode::MultiplayerCompetitive
    ) {
        deadline.start();
    } else {
        deadline.cancel();
    }
}

pub fn tick_first_move_deadline(
    mut deadline: ResMut<FirstMoveDeadline>,
    move_history: Res<MoveHistory>,
    game_over: Res<GameOverState>,
    time: Res<Time>,
    mut flag_timeout: MessageWriter<FlagTimeoutEvent>,
    game_mode: Res<GameMode>,
    barrier: Res<crate::multiplayer::types::OnlineStartBarrier>,
    session: Option<Res<crate::multiplayer::network::online_game_session::OnlineGameSession>>,
) {
    if !deadline.active {
        return;
    }
    if crate::multiplayer::types::is_online_game_mode(*game_mode)
        && !session.as_ref().is_some_and(|session| {
            barrier.is_complete(
                crate::multiplayer::network::online_game_session::numeric_game_id(&session.game_id),
            )
        })
    {
        return;
    }
    if !move_history.is_empty() || game_over.is_game_over() {
        deadline.cancel();
        return;
    }

    deadline.remaining -= time.delta_secs();
    if deadline.remaining <= 0.0 {
        deadline.remaining = 0.0;
        deadline.active = false;
        flag_timeout.write(FlagTimeoutEvent {
            flagged_player: "white".to_string(),
            remote: false,
        });
    }
}

pub fn reset_first_move_deadline(mut deadline: ResMut<FirstMoveDeadline>) {
    deadline.cancel();
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<FirstMoveDeadline>();
    app.add_systems(OnEnter(GameState::InGame), start_first_move_deadline);
    app.add_systems(OnExit(GameState::InGame), reset_first_move_deadline);
    app.add_systems(
        Update,
        tick_first_move_deadline.run_if(in_state(GameState::InGame)),
    );
}
