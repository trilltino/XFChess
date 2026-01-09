//! Promotion system - Handles pawn promotion logic
//!
//! This system detects when a pawn reaches the promotion rank and
//! handles the promotion selection.

use crate::assets::GameAssets;
use crate::game::resources::{is_promotion_move, PendingPromotion, PromotionSelected};
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::prelude::*;

/// System to detect pawns that need to be promoted
///
/// This runs after piece movement to check if a pawn reached the last rank.
/// If so, it triggers the promotion UI.
pub fn detect_pawn_promotion(
    pieces: Query<(Entity, &Piece), Changed<Piece>>,
    mut pending_promotion: ResMut<PendingPromotion>,
) {
    // Skip if a promotion is already pending
    if pending_promotion.is_active() {
        return;
    }

    for (entity, piece) in pieces.iter() {
        // Check if this is a pawn that reached the promotion rank
        // target_rank is piece.x (rank in our coordinate system)
        if is_promotion_move(piece.piece_type, piece.color, piece.x) {
            info!(
                "[PROMOTION] Pawn at ({}, {}) needs promotion",
                piece.x, piece.y
            );
            pending_promotion.start(entity, (piece.x, piece.y), piece.color);
            return;
        }
    }
}

/// System to apply the selected promotion
///
/// When the player selects a piece from the promotion UI, this system
/// changes the pawn into the selected piece type and updates the visual mesh.
pub fn apply_pawn_promotion(
    mut commands: Commands,
    mut promotion_messages: MessageReader<PromotionSelected>,
    mut pieces: Query<(&mut Piece, &Children)>,
    mut pending_promotion: ResMut<PendingPromotion>,
    game_assets: Res<GameAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for event in promotion_messages.read() {
        if let Ok((mut piece, children)) = pieces.get_mut(event.entity) {
            info!(
                "[PROMOTION] Promoting pawn at ({}, {}) to {:?}",
                event.position.0, event.position.1, event.promoted_to
            );

            // Change the pawn to the selected piece type
            piece.piece_type = event.promoted_to;

            // Despawn old mesh children
            for child in children.iter() {
                commands.entity(child).despawn();
            }

            // Create new material based on piece color
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

            // Spawn new mesh children for the promoted piece
            let meshes = &game_assets.piece_meshes;

            commands.entity(event.entity).with_children(|parent| {
                let vertical_correction = Vec3::new(0.0, -0.1, 0.0);

                match event.promoted_to {
                    PieceType::Queen => {
                        let offset = Vec3::new(-0.2, 0., -0.95);
                        let mut transform =
                            Transform::from_translation(offset + vertical_correction);
                        transform.scale = Vec3::splat(0.2);

                        parent.spawn((
                            Mesh3d(meshes.queen.clone().expect("Queen mesh not loaded")),
                            MeshMaterial3d(material),
                            transform,
                            bevy::picking::Pickable::default(),
                        ));
                    }
                    PieceType::Rook => {
                        let offset = Vec3::new(-0.1, 0., 1.8);
                        let mut transform =
                            Transform::from_translation(offset + vertical_correction);
                        transform.scale = Vec3::splat(0.2);

                        parent.spawn((
                            Mesh3d(meshes.rook.clone().expect("Rook mesh not loaded")),
                            MeshMaterial3d(material),
                            transform,
                            bevy::picking::Pickable::default(),
                        ));
                    }
                    PieceType::Bishop => {
                        let offset = Vec3::new(-0.1, 0., 0.0);
                        let mut transform =
                            Transform::from_translation(offset + vertical_correction);
                        transform.scale = Vec3::splat(0.2);

                        parent.spawn((
                            Mesh3d(meshes.bishop.clone().expect("Bishop mesh not loaded")),
                            MeshMaterial3d(material),
                            transform,
                            bevy::picking::Pickable::default(),
                        ));
                    }
                    PieceType::Knight => {
                        let offset = Vec3::new(-0.2, 0., 0.9);
                        let mut transform =
                            Transform::from_translation(offset + vertical_correction);
                        transform.scale = Vec3::splat(0.2);

                        parent.spawn((
                            Mesh3d(meshes.knight.clone().expect("Knight mesh not loaded")),
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
