//! Movement execution system

use bevy::prelude::*;
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use crate::game::resources::*;
use crate::game::components::MoveRecord;

/// System to execute piece moves
pub fn move_piece(
    mut commands: Commands,
    mut pieces_query: Query<(Entity, &mut Piece, &mut Transform)>,
    selection: Res<Selection>,
    mut current_turn: ResMut<CurrentTurn>,
    mut move_history: ResMut<MoveHistory>,
    mut click_events: EventReader<Pointer<Click>>,
    squares_query: Query<&Square, Without<Piece>>,
) {
    if !selection.is_selected() {
        return;
    }

    for event in click_events.read() {
        // Check if clicking on a valid destination square
        if let Ok(square) = squares_query.get(event.target) {
            let target = (square.x, square.y);

            if selection.possible_moves.contains(&target) {
                if let Some(selected_entity) = selection.selected_entity {
                    // First, check for captures (before any mutable borrows)
                    let mut captured_piece = None;
                    let mut captured_entity = None;
                    for (other_entity, other_piece, _) in pieces_query.iter() {
                        if other_piece.x == target.0 && other_piece.y == target.1 {
                            captured_piece = Some(other_piece.piece_type);
                            captured_entity = Some(other_entity);
                            break;
                        }
                    }

                    // Despawn captured piece if any
                    if let Some(entity) = captured_entity {
                        commands.entity(entity).despawn();
                    }

                    // Now mutate the selected piece
                    if let Ok((_, mut piece, _)) = pieces_query.get_mut(selected_entity) {
                        // Record the move
                        let move_record = MoveRecord {
                            piece_type: piece.piece_type,
                            piece_color: piece.color,
                            from: (piece.x, piece.y),
                            to: target,
                            captured: captured_piece,
                            is_castling: false,
                            is_en_passant: false,
                            is_check: false,
                            is_checkmate: false,
                        };
                        move_history.add_move(move_record);

                        // Execute the move
                        piece.x = target.0;
                        piece.y = target.1;

                        info!("Moved {:?} to ({}, {})", piece.piece_type, target.0, target.1);

                        // Switch turns
                        current_turn.switch();
                        info!("Turn: {:?} - Move #{}", current_turn.color, current_turn.move_number);
                    }
                }
            }
        }
    }
}
