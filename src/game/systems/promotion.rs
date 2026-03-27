//! Pawn promotion system.

use crate::game::resources::{is_promotion_move, PendingPromotion, PromotionSelected};
use crate::rendering::pieces::{Piece, PieceColor, PieceType, PIECE_MESH_SCALE};
use bevy::prelude::*;

/// Detects pawns that need promotion.
pub fn detect_pawn_promotion(
    pieces: Query<(Entity, &Piece), Changed<Piece>>,
    mut pending_promotion: ResMut<PendingPromotion>,
) {
    if pending_promotion.is_active() {
        return;
    }

    for (entity, piece) in pieces.iter() {
        if is_promotion_move(piece.piece_type, piece.color, piece.y) {
            info!(
                "[PROMOTION] Pawn at ({}, {}) needs promotion",
                piece.x, piece.y
            );
            pending_promotion.start(entity, (piece.x, piece.y), piece.color);
            return;
        }
    }
}

/// Applies the selected promotion.
pub fn apply_pawn_promotion(
    mut commands: Commands,
    mut promotion_messages: MessageReader<PromotionSelected>,
    mut pieces: Query<(&mut Piece, &Children)>,
    mut pending_promotion: ResMut<PendingPromotion>,
    piece_meshes: Res<crate::rendering::pieces::PieceMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for event in promotion_messages.read() {
        if let Ok((mut piece, children)) = pieces.get_mut(event.entity) {
            info!(
                "[PROMOTION] Promoting pawn at ({}, {}) to {:?}",
                event.position.0, event.position.1, event.promoted_to
            );

            piece.piece_type = event.promoted_to;

            for child in children.iter() {
                commands.entity(child).despawn();
            }

            let material = if piece.color == PieceColor::White {
                materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    ..default()
                })
            } else {
                materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    ..default()
                })
            };

            commands.entity(event.entity).with_children(|parent| {
                let mesh = piece_meshes.get(event.promoted_to, piece.color);
                match event.promoted_to {
                    PieceType::Queen | PieceType::Rook | PieceType::Bishop | PieceType::Knight => {
                        parent.spawn((
                            Mesh3d(mesh),
                            MeshMaterial3d(material),
                            Transform::from_scale(Vec3::splat(PIECE_MESH_SCALE)),
                            bevy::picking::Pickable::default(),
                        ));
                    }
                    _ => {}
                }
            });
        }

        // Clear the pending promotion
        pending_promotion.clear();
    }
}
