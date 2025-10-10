//! Game logic systems for phase updates and timing

use bevy::prelude::*;
use crate::rendering::pieces::{Piece, PieceColor};
use crate::game::resources::*;
use crate::game::components::GamePhase;

/// System to update game phase (check, checkmate, etc.)
pub fn update_game_phase(
    _game_phase: ResMut<CurrentGamePhase>,
    _current_turn: Res<CurrentTurn>,
    _pieces_query: Query<&Piece>,
) {
    // TODO: Implement check/checkmate detection
    // This would involve:
    // 1. Finding the king of the current player
    // 2. Checking if any opponent piece can attack it (check)
    // 3. If in check, checking if any move can get out of check
    // 4. If no moves available, it's checkmate
}

/// System to update game timer
pub fn update_game_timer(
    mut timer: ResMut<GameTimer>,
    time: Res<Time>,
    current_turn: Res<CurrentTurn>,
    game_phase: Res<CurrentGamePhase>,
) {
    if !timer.is_running || game_phase.0 != GamePhase::Playing {
        return;
    }

    let delta = time.delta_secs();
    match current_turn.color {
        PieceColor::White => {
            timer.white_time_left -= delta;
            if timer.white_time_left <= 0.0 {
                timer.white_time_left = 0.0;
                timer.is_running = false;
                info!("White ran out of time!");
            }
        }
        PieceColor::Black => {
            timer.black_time_left -= delta;
            if timer.black_time_left <= 0.0 {
                timer.black_time_left = 0.0;
                timer.is_running = false;
                info!("Black ran out of time!");
            }
        }
    }
}
