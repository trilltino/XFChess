//! Pawn promotion system.

use crate::game::resources::{is_promotion_move, PendingPromotion, PromotionSelected};
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
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
                // Use the same scale and offsets as the main piece spawning system
                // These must match the constants in rendering/pieces/pieces.rs
                const PIECE_MESH_SCALE: f32 = 0.18;

                // Per-piece offsets (must match pieces.rs exactly)
                const QUEEN_OFFSET: Vec3 = Vec3::new(-0.2, -0.5, -0.95);
                const ROOK_OFFSET: Vec3 = Vec3::new(-0.1, -0.5, 1.8);
                const BISHOP_OFFSET: Vec3 = Vec3::new(-0.1, -0.5, 0.0);
                const KNIGHT_OFFSET: Vec3 = Vec3::new(-0.2, -0.5, 0.9);

                match event.promoted_to {
                    PieceType::Queen => {
                        let mut transform = Transform::from_translation(QUEEN_OFFSET);
                        transform.scale = Vec3::splat(PIECE_MESH_SCALE);

                        parent.spawn((
                            Mesh3d(piece_meshes.queen.clone()),
                            MeshMaterial3d(material),
                            transform,
                            bevy::picking::Pickable::default(),
                        ));
                    }
                    PieceType::Rook => {
                        let mut transform = Transform::from_translation(ROOK_OFFSET);
                        transform.scale = Vec3::splat(PIECE_MESH_SCALE);

                        parent.spawn((
                            Mesh3d(piece_meshes.rook.clone()),
                            MeshMaterial3d(material),
                            transform,
                            bevy::picking::Pickable::default(),
                        ));
                    }
                    PieceType::Bishop => {
                        let mut transform = Transform::from_translation(BISHOP_OFFSET);
                        transform.scale = Vec3::splat(PIECE_MESH_SCALE);

                        parent.spawn((
                            Mesh3d(piece_meshes.bishop.clone()),
                            MeshMaterial3d(material),
                            transform,
                            bevy::picking::Pickable::default(),
                        ));
                    }
                    PieceType::Knight => {
                        // Knights have two mesh parts
                        let mut transform = Transform::from_translation(KNIGHT_OFFSET);
                        transform.scale = Vec3::splat(PIECE_MESH_SCALE);

                        parent.spawn((
                            Mesh3d(piece_meshes.knight_1.clone()),
                            MeshMaterial3d(material.clone()),
                            transform,
                            bevy::picking::Pickable::default(),
                        ));

                        parent.spawn((
                            Mesh3d(piece_meshes.knight_2.clone()),
                            MeshMaterial3d(material),
                            transform,
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
