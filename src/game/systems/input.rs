//! Input handling systems for piece selection and clicks

use bevy::prelude::*;
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use crate::game::resources::*;
use crate::game::rules::{BoardState, get_possible_moves};

/// System to handle piece selection when clicking on squares
pub fn handle_piece_selection(
    mut selection: ResMut<Selection>,
    current_turn: Res<CurrentTurn>,
    pieces_query: Query<(Entity, &Piece, &Transform)>,
    mut click_events: EventReader<Pointer<Click>>,
    squares_query: Query<(&Square, &Transform), Without<Piece>>,
) {
    for event in click_events.read() {
        let clicked_entity = event.target;

        // Try to find if a piece was clicked
        if let Ok((entity, piece, _)) = pieces_query.get(clicked_entity) {
            // Only allow selecting pieces of the current player's color
            if piece.color == current_turn.color {
                // Calculate possible moves
                let board_state = build_board_state(&pieces_query);
                let possible_moves = get_possible_moves(
                    piece.piece_type,
                    piece.color,
                    (piece.x, piece.y),
                    &board_state,
                    false, // TODO: Track has_moved
                );

                selection.selected_entity = Some(entity);
                selection.selected_position = Some((piece.x, piece.y));
                selection.possible_moves = possible_moves;

                info!("Selected {:?} {:?} at ({}, {})", piece.color, piece.piece_type, piece.x, piece.y);
                info!("Possible moves: {:?}", selection.possible_moves);
            }
        }
        // Try to find if a square was clicked (for moving)
        else if let Ok((square, _)) = squares_query.get(clicked_entity) {
            if selection.is_selected() {
                let target = (square.x, square.y);
                // Check if this is a valid move
                if selection.possible_moves.contains(&target) {
                    info!("Valid move to ({}, {})", target.0, target.1);
                    // The actual move will be handled by move_piece system
                } else {
                    info!("Invalid move to ({}, {})", target.0, target.1);
                    selection.clear();
                }
            }
        }
    }
}

/// System to clear selection when clicking empty space
pub fn clear_selection_on_empty_click(
    mut selection: ResMut<Selection>,
    mut click_events: EventReader<Pointer<Click>>,
    pieces_query: Query<Entity, With<Piece>>,
    squares_query: Query<Entity, With<Square>>,
) {
    for event in click_events.read() {
        let clicked_entity = event.target;

        // If clicked something that's not a piece or square, clear selection
        if pieces_query.get(clicked_entity).is_err() && squares_query.get(clicked_entity).is_err() {
            selection.clear();
        }
    }
}

/// Helper function to build board state from current pieces
fn build_board_state(pieces_query: &Query<(Entity, &Piece, &Transform)>) -> BoardState {
    let pieces = pieces_query
        .iter()
        .map(|(entity, piece, _)| (entity, *piece, (piece.x, piece.y)))
        .collect();

    BoardState { pieces }
}
