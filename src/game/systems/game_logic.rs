//! Game logic systems for phase updates and timing

use bevy::prelude::*;
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::game::resources::*;
use crate::game::components::{GamePhase, HasMoved};
use crate::game::rules::{BoardState, get_possible_moves};

/// System to update game phase (check, checkmate, etc.)
///
/// This system runs after each move to detect:
/// - Check: King is under attack
/// - Checkmate: King is under attack with no legal moves
/// - Stalemate: No legal moves available but not in check
pub fn update_game_phase(
    mut game_phase: ResMut<CurrentGamePhase>,
    mut game_over: ResMut<GameOverState>,
    current_turn: Res<CurrentTurn>,
    pieces_query: Query<(Entity, &Piece, &HasMoved)>,
) {
    let previous_phase = game_phase.0;

    // Build current board state
    let board_state = BoardState {
        pieces: pieces_query
            .iter()
            .map(|(entity, piece, _)| (entity, *piece, (piece.x, piece.y)))
            .collect(),
    };

    // Find the current player's king
    let king_pos = pieces_query
        .iter()
        .find(|(_, piece, _)| {
            piece.piece_type == PieceType::King && piece.color == current_turn.color
        })
        .map(|(_, piece, _)| (piece.x, piece.y));

    if let Some(king_position) = king_pos {
        // Check if the king is under attack
        let is_in_check = is_square_under_attack(king_position, current_turn.color, &board_state);

        if is_in_check {
            // Check if there are any legal moves to get out of check
            let has_legal_moves = has_any_legal_moves(current_turn.color, &pieces_query, &board_state);

            if has_legal_moves {
                // Only log when check state changes
                if previous_phase != GamePhase::Check {
                    game_phase.0 = GamePhase::Check;
                    info!("[GAME] ========== CHECK DETECTED ==========");
                    info!("[GAME] {:?} King at ({}, {}) is under attack!",
                        current_turn.color, king_position.0, king_position.1);
                    info!("[GAME] {:?} must defend or move king to escape check", current_turn.color);
                } else {
                    game_phase.0 = GamePhase::Check;
                }
            } else {
                game_phase.0 = GamePhase::Checkmate;
                // Set game over state - opponent wins
                *game_over = match current_turn.color {
                    PieceColor::White => GameOverState::BlackWon,
                    PieceColor::Black => GameOverState::WhiteWon,
                };
                info!("[GAME] ========== CHECKMATE! ==========");
                info!("[GAME] {:?} King at ({}, {}) is in checkmate!",
                    current_turn.color, king_position.0, king_position.1);
                info!("[GAME] {} - {}",
                    game_over.message(),
                    game_over.winner().map(|c| format!("{:?} WINS!", c)).unwrap_or_default());
                info!("[GAME] Final Move: #{}", current_turn.move_number);
            }
        } else {
            // Not in check - check for stalemate
            let has_legal_moves = has_any_legal_moves(current_turn.color, &pieces_query, &board_state);

            if has_legal_moves {
                // Only log when transitioning out of check
                if previous_phase == GamePhase::Check {
                    game_phase.0 = GamePhase::Playing;
                    info!("[GAME] Check escaped! Game continues normally");
                } else {
                    game_phase.0 = GamePhase::Playing;
                }
            } else {
                game_phase.0 = GamePhase::Stalemate;
                *game_over = GameOverState::Stalemate;
                info!("[GAME] ========== STALEMATE! ==========");
                info!("[GAME] {:?} has no legal moves but is not in check", current_turn.color);
                info!("[GAME] Game ends in a DRAW - {}", game_over.message());
                info!("[GAME] Final Move: #{}", current_turn.move_number);
            }
        }

        // Phase transition already logged in specific cases above
    } else {
        warn!("[GAME] CRITICAL: Cannot find {:?} King! Board state may be corrupted", current_turn.color);
        warn!("[GAME] Board has {} total pieces", board_state.pieces.len());
    }
}

/// Helper function to check if a square is under attack by the opponent
fn is_square_under_attack(
    position: (u8, u8),
    defending_color: PieceColor,
    board_state: &BoardState,
) -> bool {
    let opponent_color = match defending_color {
        PieceColor::White => PieceColor::Black,
        PieceColor::Black => PieceColor::White,
    };

    // Check all opponent pieces to see if any can attack this square
    for piece in board_state.get_pieces_by_color(opponent_color) {
        let possible_moves = get_possible_moves(
            piece.piece_type,
            piece.color,
            (piece.x, piece.y),
            board_state,
            true, // Assume all pieces have moved for attack detection
        );

        if possible_moves.contains(&position) {
            // Square is under attack (only log in verbose mode to avoid spam)
            return true;
        }
    }

    false
}

/// Helper function to check if the current player has any legal moves
fn has_any_legal_moves(
    color: PieceColor,
    pieces_query: &Query<(Entity, &Piece, &HasMoved)>,
    board_state: &BoardState,
) -> bool {
    let mut total_moves = 0;
    let mut pieces_checked = 0;

    // Check each piece of the current color
    for (_, piece, has_moved) in pieces_query.iter() {
        if piece.color == color {
            pieces_checked += 1;
            let possible_moves = get_possible_moves(
                piece.piece_type,
                piece.color,
                (piece.x, piece.y),
                board_state,
                has_moved.moved,
            );

            total_moves += possible_moves.len();

            if !possible_moves.is_empty() {
                // Found legal moves (no need to log every check)
                return true;
            }
        }
    }

    // No legal moves found (logged in checkmate/stalemate detection)
    false
}

/// System to update game timer
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
                info!("[TIMER] White ran out of time! Final: W:{:.1}s B:{:.1}s",
                    timer.white_time_left, timer.black_time_left);
                info!("[TIMER] {}", game_over.message());
                info!("[TIMER] Move #{} | Black WINS by timeout!", current_turn.move_number);
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
                info!("[TIMER] Black ran out of time! Final: W:{:.1}s B:{:.1}s",
                    timer.white_time_left, timer.black_time_left);
                info!("[TIMER] {}", game_over.message());
                info!("[TIMER] Move #{} | White WINS by timeout!", current_turn.move_number);
            }
        }
    }
}
