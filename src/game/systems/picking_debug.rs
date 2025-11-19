//! Picking debug system
//!
//! Provides comprehensive debugging for pointer interactions and picking events.

use crate::core::GameState;
use crate::game::resources::Selection;
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use bevy::ecs::message::MessageReader;
use bevy::picking::events::Pointer;
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;

/// Debug system that logs all pointer click events using MessageReader
///
/// This system listens to all Pointer<Click> messages to see if
/// picking is working at all, regardless of which entity is clicked.
pub fn debug_all_clicks(
    mut click_reader: MessageReader<Pointer<Click>>,
    pieces: Query<(Entity, &Piece)>,
    squares: Query<(Entity, &Square)>,
    selection: Res<Selection>,
    all_entities: Query<Entity>,
) {
    for event in click_reader.read() {
        warn!("[PICKING_DEBUG] ========== CLICK MESSAGE RECEIVED ==========");
        warn!("[PICKING_DEBUG] Target entity: {:?}", event.entity);
        warn!("[PICKING_DEBUG] Pointer ID: {:?}", event.pointer_id);
        warn!("[PICKING_DEBUG] Button: {:?}", event.event.button);

        // Check if it's a piece (query without PointerInteraction requirement)
        if let Ok((entity, piece)) = pieces.get(event.entity) {
            warn!(
                "[PICKING_DEBUG] CLICKED PIECE: Entity {:?} - {:?} {:?} at ({}, {})",
                entity, piece.color, piece.piece_type, piece.x, piece.y
            );
            warn!(
                "[PICKING_DEBUG] Current selection: {:?}",
                selection.selected_entity
            );
        } else if let Ok((entity, square)) = squares.get(event.entity) {
            warn!(
                "[PICKING_DEBUG] CLICKED SQUARE: Entity {:?} - Square at ({}, {})",
                entity, square.x, square.y
            );
            warn!(
                "[PICKING_DEBUG] Current selection: {:?}",
                selection.selected_entity
            );
        } else {
            warn!(
                "[PICKING_DEBUG] CLICKED ENTITY: {:?} (not a piece or square)",
                event.entity
            );
            // Check if entity exists at all
            if all_entities.get(event.entity).is_ok() {
                warn!("[PICKING_DEBUG] Entity exists but doesn't have Piece or Square component");
            } else {
                warn!("[PICKING_DEBUG] Entity doesn't exist!");
            }
        }
        warn!("[PICKING_DEBUG] ========================================");
    }
}

/// Debug system that monitors PointerInteraction component states
///
/// Checks if PointerInteraction is being updated when hovering/clicking.
pub fn debug_pointer_interactions(
    pieces: Query<(Entity, &Piece, &PointerInteraction), (With<Piece>, With<PointerInteraction>)>,
    squares: Query<
        (Entity, &Square, &PointerInteraction),
        (With<Square>, With<PointerInteraction>),
    >,
    time: Res<Time>,
    mut last_check: Local<f32>,
) {
    // Only check periodically (every 2 seconds) to avoid spam
    *last_check += time.delta_secs();
    if *last_check < 2.0 {
        return;
    }
    *last_check = 0.0;

    // Check for any pieces with active interactions (hovered or clicked)
    for (entity, piece, interaction) in pieces.iter() {
        if let Some((hit_entity, _hit_data)) = interaction.get_nearest_hit() {
            if *hit_entity == entity {
                warn!(
                    "[PICKING_DEBUG] Piece {:?} {:?} at ({}, {}) is being hovered/clicked",
                    piece.color, piece.piece_type, piece.x, piece.y
                );
            }
        }
    }

    // Check for any squares with active interactions
    for (entity, square, interaction) in squares.iter() {
        if let Some((hit_entity, _hit_data)) = interaction.get_nearest_hit() {
            if *hit_entity == entity {
                warn!(
                    "[PICKING_DEBUG] Square at ({}, {}) is being hovered/clicked",
                    square.x, square.y
                );
            }
        }
    }
}

/// Debug system that checks if entities have required components for picking
pub fn debug_picking_setup(
    pieces: Query<(Entity, &Piece, Option<&PointerInteraction>), (With<Piece>, Without<Square>)>,
    squares: Query<(Entity, &Square, Option<&PointerInteraction>), (With<Square>, Without<Piece>)>,
    cameras: Query<(Entity, &Camera3d)>,
    time: Res<Time>,
    mut last_check: Local<f32>,
) {
    // Only check periodically (every 10 seconds) to avoid spam
    *last_check += time.delta_secs();
    if *last_check < 10.0 {
        return;
    }
    *last_check = 0.0;

    warn!("[PICKING_DEBUG] ========== PICKING SETUP CHECK ==========");

    // Check cameras
    warn!(
        "[PICKING_DEBUG] Cameras in scene: {}",
        cameras.iter().count()
    );
    for (entity, _camera) in cameras.iter() {
        warn!("[PICKING_DEBUG]   Camera entity: {:?}", entity);
    }

    let mut pieces_with_interaction = 0;
    let mut pieces_without_interaction = 0;

    for (entity, piece, interaction) in pieces.iter() {
        if interaction.is_some() {
            pieces_with_interaction += 1;
        } else {
            pieces_without_interaction += 1;
            warn!("[PICKING_DEBUG] PIECE MISSING PointerInteraction: Entity {:?} - {:?} {:?} at ({}, {})", 
                entity, piece.color, piece.piece_type, piece.x, piece.y);
        }
    }

    warn!(
        "[PICKING_DEBUG] Pieces with PointerInteraction: {}",
        pieces_with_interaction
    );
    warn!(
        "[PICKING_DEBUG] Pieces without PointerInteraction: {}",
        pieces_without_interaction
    );

    let mut squares_with_interaction = 0;
    let mut squares_without_interaction = 0;

    for (entity, square, interaction) in squares.iter() {
        if interaction.is_some() {
            squares_with_interaction += 1;
        } else {
            squares_without_interaction += 1;
            warn!("[PICKING_DEBUG] SQUARE MISSING PointerInteraction: Entity {:?} - Square at ({}, {})", 
                entity, square.x, square.y);
        }
    }

    warn!(
        "[PICKING_DEBUG] Squares with PointerInteraction: {}",
        squares_with_interaction
    );
    warn!(
        "[PICKING_DEBUG] Squares without PointerInteraction: {}",
        squares_without_interaction
    );

    if pieces_without_interaction > 0 || squares_without_interaction > 0 {
        error!("[PICKING_DEBUG] ERROR: Some entities are missing PointerInteraction component!");
        error!("[PICKING_DEBUG] This will prevent picking from working!");
    }

    warn!("[PICKING_DEBUG] ========================================");
}

/// Plugin for picking debug systems
pub struct PickingDebugPlugin;

impl Plugin for PickingDebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                debug_all_clicks,
                debug_pointer_interactions,
                debug_picking_setup,
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}
