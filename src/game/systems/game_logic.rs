//! Game logic systems for phase updates and timing
//!
//! These systems handle core chess game logic including:
//! - Game phase detection (check, checkmate, stalemate)
//! - Game timer management with Fischer increment
//! - Game over state transitions
//!
//! # System Execution Order
//!
//! These systems run in the `Execution` system set, after move validation
//! and before visual updates. This ensures game state is updated before
//! rendering changes.

use crate::game::components::GamePhase;
use crate::game::resources::*;
use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;
use chess_engine::{get_game_state, is_in_check, STATE_CHECKMATE, STATE_PLAYING, STATE_STALEMATE};

/// System to update game phase (check, checkmate, etc.)
///
/// This system runs after each move to detect:
/// - **Check**: King is under attack
/// - **Checkmate**: King is under attack with no legal moves
/// - **Stalemate**: No legal moves available but not in check
///
/// # Execution Order
///
/// Runs in `GameSystems::Execution` set, after move validation and
/// before visual updates.
///
/// # System Parameters
///
/// Uses individual resources for clarity. Consider using [`GameStateParams`]
/// if you need access to multiple game state resources.
///
/// # Errors
///
/// Handles unknown engine states gracefully by defaulting to Playing phase.
///
/// # TempleOS Mode
///
/// In TempleOS mode, no pieces are spawned, so game phase checks are skipped
/// to prevent false stalemate detection on an empty board.
pub fn update_game_phase(
    mut game_phase: ResMut<CurrentGamePhase>,
    mut game_over: ResMut<GameOverState>,
    current_turn: Res<CurrentTurn>,
    pieces_query: Query<(
        Entity,
        &crate::rendering::pieces::Piece,
        &crate::game::components::HasMoved,
    )>,
    mut engine: ResMut<ChessEngine>,
    view_mode: Res<crate::game::view_mode::ViewMode>,
) {
    // Skip game phase checks in TempleOS mode (no pieces = empty board)
    if *view_mode == crate::game::view_mode::ViewMode::TempleOS {
        debug!("[GAME] Skipping game phase check - TempleOS mode (no pieces)");
        return;
    }

    let previous_phase = game_phase.0;
    let piece_count = pieces_query.iter().count();

    // Only log if piece count changed - reduces log spam
    // We could store the last count in a local request, but for now just trace level is enough
    trace!(
        "[GAME] Updating game phase - {} pieces on board",
        piece_count
    );

    // Sync ECS → Engine before checking game state
    engine.sync_ecs_to_engine(&pieces_query, &current_turn);

    // Get game state from engine
    let engine_color = ChessEngine::piece_color_to_engine(current_turn.color);
    let engine_state = get_game_state(&mut engine.game, engine_color);

    // Check if king is in check (for check detection)
    let in_check = is_in_check(&engine.game, engine_color);

    match engine_state {
        STATE_CHECKMATE => {
            game_phase.0 = GamePhase::Checkmate;
            // Set game over state - opponent wins
            *game_over = match current_turn.color {
                PieceColor::White => GameOverState::BlackWon,
                PieceColor::Black => GameOverState::WhiteWon,
            };
            info!("[GAME] ========== CHECKMATE! ==========");
            info!("[GAME] {:?} is in checkmate!", current_turn.color);
            info!(
                "[GAME] {} - {}",
                game_over.message(),
                game_over
                    .winner()
                    .map(|c| format!("{:?} WINS!", c))
                    .unwrap_or_default()
            );
            info!("[GAME] Final Move: #{}", current_turn.move_number);
        }
        STATE_STALEMATE => {
            game_phase.0 = GamePhase::Stalemate;
            *game_over = GameOverState::Stalemate;
            info!("[GAME] ========== STALEMATE! ==========");
            info!(
                "[GAME] {:?} has no legal moves but is not in check",
                current_turn.color
            );
            info!("[GAME] Game ends in a DRAW - {}", game_over.message());
            info!("[GAME] Final Move: #{}", current_turn.move_number);
        }
        STATE_PLAYING => {
            if in_check {
                // King is in check but has legal moves
                if previous_phase != GamePhase::Check {
                    game_phase.0 = GamePhase::Check;
                    info!("[GAME] ========== CHECK DETECTED ==========");
                    info!("[GAME] {:?} King is under attack!", current_turn.color);
                    info!(
                        "[GAME] {:?} must defend or move king to escape check",
                        current_turn.color
                    );
                } else {
                    game_phase.0 = GamePhase::Check;
                }
            } else {
                // Not in check, game continues normally
                if previous_phase == GamePhase::Check {
                    game_phase.0 = GamePhase::Playing;
                    info!("[GAME] Check escaped! Game continues normally");
                } else {
                    game_phase.0 = GamePhase::Playing;
                }
            }
        }
        _ => {
            warn!(
                "[GAME] Unknown engine state: {}. Defaulting to Playing phase.",
                engine_state
            );
            game_phase.0 = GamePhase::Playing;
        }
    }
}

/// System to update game timer with Fischer increment support
///
/// Decrements the current player's time each frame and checks for timeout.
/// Applies Fischer increment after moves (handled by move execution systems).
///
/// # Execution Order
///
/// Runs in `GameSystems::Execution` set, after game phase updates.
///
/// # Timer Behavior
///
/// - Timer only runs during `GamePhase::Playing`
/// - Timer pauses when game is over or paused
/// - Timeout detection sets `GameOverState` to appropriate win condition
///
/// # System Parameters
///
/// Uses individual resources. Consider using [`GameHistoryParams`] if you
/// also need access to move history.
pub fn update_game_timer(
    mut timer: ResMut<GameTimer>,
    mut game_over: ResMut<GameOverState>,
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
            let time_before = timer.white_time_left;
            timer.white_time_left -= delta;

            // Log time warnings
            if time_before > 10.0 && timer.white_time_left <= 10.0 {
                warn!("[TIMER] White has 10 seconds remaining!");
            } else if time_before > 30.0 && timer.white_time_left <= 30.0 {
                info!("[TIMER] White has 30 seconds remaining");
            }

            if timer.white_time_left <= 0.0 {
                timer.white_time_left = 0.0;
                timer.is_running = false;
                *game_over = GameOverState::BlackWonByTime;
                info!("[TIMER] ========== TIME OUT! ==========");
                info!(
                    "[TIMER] White ran out of time! Final: W:{:.1}s B:{:.1}s",
                    timer.white_time_left, timer.black_time_left
                );
                info!("[TIMER] {}", game_over.message());
                info!(
                    "[TIMER] Move #{} | Black WINS by timeout!",
                    current_turn.move_number
                );
            }
        }
        PieceColor::Black => {
            let time_before = timer.black_time_left;
            timer.black_time_left -= delta;

            // Log time warnings
            if time_before > 10.0 && timer.black_time_left <= 10.0 {
                warn!("[TIMER] Black has 10 seconds remaining!");
            } else if time_before > 30.0 && timer.black_time_left <= 30.0 {
                info!("[TIMER] Black has 30 seconds remaining");
            }

            if timer.black_time_left <= 0.0 {
                timer.black_time_left = 0.0;
                timer.is_running = false;
                *game_over = GameOverState::WhiteWonByTime;
                info!("[TIMER] ========== TIME OUT! ==========");
                info!(
                    "[TIMER] Black ran out of time! Final: W:{:.1}s B:{:.1}s",
                    timer.white_time_left, timer.black_time_left
                );
                info!("[TIMER] {}", game_over.message());
                info!(
                    "[TIMER] Move #{} | White WINS by timeout!",
                    current_turn.move_number
                );
            }
        }
    }
}

/// System to transition game state when game is over
///
/// Watches for changes in [`GameOverState`] and updates the Bevy State machine.
/// This ensures systems that should only run during active gameplay are stopped.
pub fn check_game_over_state(
    game_over: Res<GameOverState>,
    state: Res<State<crate::core::GameState>>,
    mut next_state: ResMut<NextState<crate::core::GameState>>,
) {
    // Only transition if we are currently InGame and the game is effectively over
    if *state.get() == crate::core::GameState::InGame && game_over.is_game_over() {
        info!(
            "[GAME] Game over condition met ({:?}) - transitioning to GameOver state",
            *game_over
        );
        next_state.set(crate::core::GameState::GameOver);
    }
}
