use crate::engine::board_state::ChessEngine;
use crate::game::components::{HasMoved, Piece, PieceType};
use crate::game::events::NetworkMoveEvent;
use crate::game::resources::{
    CapturedPieces, GameSounds, MoveHistory, PendingTurnAdvance, Selection,
};
use crate::game::systems::shared::{execute_move, CapturedTarget, MoveContext};
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
    children_query: Query<&Children>,
    material_query: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for event in events.read() {
        info!(
            "[NETWORK_MOVE] Processing move: {:?} -> {:?}",
            event.from, event.to
        );

        // 1. Find Source Entity and Piece Data
        let source_data = pieces_query
            .iter()
            .find(|(_, piece, _)| piece.x == event.from.0 && piece.y == event.from.1)
            .map(|(e, p, _)| (e, *p));

        if let Some((entity, piece)) = source_data {
            // 2. Find Potential Capture
            let capture_data = pieces_query
                .iter()
                .find(|(_, p, _)| p.x == event.to.0 && p.y == event.to.1)
                .map(|(e, p, _)| (e, *p));

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

            // 3. Determine first-move status before execute_move borrows the query
            let was_first_move = if let Ok((_, _, has_moved)) = pieces_query.get(entity) {
                !has_moved.moved
            } else {
                false
            };

            // 4. Map Promotion Piece
            let promotion_type = event.promotion.and_then(PieceType::from_char);

            // 5. Execute Move
            let ctx = MoveContext {
                origin: "network_move",
                entity,
                piece,
                target: event.to,
                capture: capture_target,
                promotion: promotion_type,
                was_first_move,
                remote: true,
                move_sound: game_sounds.as_ref().map(|s| s.move_piece.clone()),
                capture_sound: game_sounds.as_ref().map(|s| s.capture_piece.clone()),
            };

            execute_move(
                &ctx,
                &mut commands,
                &mut pending_turn,
                &mut move_history,
                &mut captured_pieces,
                &mut engine,
                &mut pieces_query,
                None, // No MoveMadeEvent writer — avoid local echo
                None, // BoardStateSync — network moves don't broadcast
                &children_query,
                &material_query,
                &mut materials,
            );

            // 6. Update Selection (Clear if we moved selected piece)
            if let Some(selected_entity) = selection.selected_entity {
                if selected_entity == entity {
                    selection.selected_entity = None;
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
