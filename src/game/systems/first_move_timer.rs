//! Starts and ticks the 30-second "must play the first move" grace period
//! for online games (`FirstMoveDeadline`). See
//! `crate::game::resources::first_move_deadline` for the resource itself.

use crate::core::{GameMode, GameState};
use crate::game::events::FlagTimeoutEvent;
use crate::game::resources::{FirstMoveDeadline, GameOverState, MoveHistory};
use bevy::prelude::*;

/// Starts the first-move countdown on entering `InGame`, for online modes only.
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

/// Ticks the countdown while active and no move has been played yet.
/// Cancels itself once the first move lands; fires `FlagTimeoutEvent` (with
/// no `flagged_player` side meaningful — resolved as an abort, not a win,
/// by `handle_flag_timeout_events`, since `MoveHistory` is still empty) if
/// the deadline is reached first.
pub fn tick_first_move_deadline(
    mut deadline: ResMut<FirstMoveDeadline>,
    move_history: Res<MoveHistory>,
    game_over: Res<GameOverState>,
    time: Res<Time>,
    mut flag_timeout: MessageWriter<FlagTimeoutEvent>,
) {
    if !deadline.active {
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

/// Resets the deadline on leaving `InGame` so a stale countdown never leaks
/// into the next game.
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
