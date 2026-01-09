use crate::game::components::HasMoved;
use crate::game::events::NetworkMoveEvent;
use crate::game::resources::{
    CapturedPieces, ChessEngine, GameSounds, MoveHistory, PendingTurnAdvance, Selection,
};
use crate::game::systems::shared::{execute_move, CapturedTarget};
use crate::rendering::pieces::Piece;
use bevy::prelude::*;

/// Handle network move events by executing them on the local board
pub fn handle_network_moves(
    mut events: MessageReader<NetworkMoveEvent>,
    mut commands: Commands,
    mut pieces_query: Query<(Entity, &mut Piece, &mut HasMoved)>,
    mut selection: ResMut<Selection>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut move_history: ResMut<MoveHistory>,
    mut captured_pieces: ResMut<CapturedPieces>,
    mut engine: ResMut<ChessEngine>,
    game_sounds: Option<Res<GameSounds>>,
) {
    for event in events.read() {
        info!(
            "[NETWORK_MOVE] Processing move: {:?} -> {:?}",
            event.from, event.to
        );

        // 1. Find Source Entity and Piece Data
        // We iterate to find the connection, cloning the data to release the borrow
        let source_data = pieces_query
            .iter()
            .find(|(_, piece, _)| piece.x == event.from.0 && piece.y == event.from.1)
            .map(|(e, p, _)| (e, p.clone()));

        if let Some((entity, piece)) = source_data {
            // 2. Find Potential Capture
            let capture_data = pieces_query
                .iter()
                .find(|(_, p, _)| p.x == event.to.0 && p.y == event.to.1)
                .map(|(e, p, _)| (e, p.clone()));

            let capture_target = if let Some((cap_entity, cap_piece)) = capture_data {
                if cap_piece.color != piece.color {
                    Some(CapturedTarget {
                        entity: cap_entity,
                        piece_type: cap_piece.piece_type,
                        color: cap_piece.color,
                    })
                } else {
                    warn!("[NETWORK_MOVE] Attempted move to occupied square of same color!");
                    None
                }
            } else {
                None
            };

            // 3. Determine if it was first move (for history/notation, though logic usually checks HasMoved before update)
            // Wait, execute_move uses 'was_first_move' arg?
            // Yes. I need to know if it was first move.
            // I can check HasMoved component.
            // But I can't access it while pieces_query is borrowed by execute_move?
            // I should get it NOW.
            let was_first_move = if let Ok((_, _, has_moved)) = pieces_query.get(entity) {
                !has_moved.moved
            } else {
                false
            };

            // 4. Execute Move
            execute_move(
                "network_move",
                &mut commands,
                entity,
                piece,
                event.to,
                capture_target,
                was_first_move,
                &mut pending_turn,
                &mut move_history,
                &mut captured_pieces,
                &mut engine,
                &mut pieces_query,
                game_sounds.as_ref().map(|s| s.move_piece.clone()),
                game_sounds.as_ref().map(|s| s.capture_piece.clone()),
            );

            // 5. Update Selection (Clear if we moved selected piece)
            // If the moved piece was selected, clear selection to avoid weird state
            if let Some(selected_entity) = selection.selected_entity {
                if selected_entity == entity {
                    // We moved the piece we had selected. Clear selection.
                    selection.selected_entity = None;
                    // We should also strip SelectedPiece component?
                    // clear_selection_state helper handles this.
                    // But I don't have access to SelectedPiece query here or helper.
                    // We can just remove the component manually.
                    commands
                        .entity(entity)
                        .remove::<crate::game::components::SelectedPiece>();
                }
            }
        } else {
            warn!("[NETWORK_MOVE] Source piece not found at {:?}", event.from);
        }
    }
}
