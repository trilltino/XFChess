use crate::game::components::{HasMoved, SelectedPiece};
use crate::game::resources::{
    CapturedPieces, ChessEngine, CurrentTurn, GameOverState, GameSounds, MoveHistory,
    PendingTurnAdvance, Selection,
};
use crate::game::systems::shared::{execute_move, CapturedTarget};
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use crate::safe_observer;
use bevy::audio::AudioSource;
use bevy::picking::events::{Click, Drag, DragEnd, DragStart, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

/// Helper to check if primary button (left click) was used
fn is_primary(button: PointerButton) -> bool {
    matches!(button, PointerButton::Primary)
}

/// Helper to clear ECS selection markers
fn reset_selected_markers(
    commands: &mut Commands,
    selected_pieces: &Query<Entity, With<SelectedPiece>>,
) {
    for entity in selected_pieces.iter() {
        commands.entity(entity).remove::<SelectedPiece>();
    }
}

/// Helper to clear all selection state
fn clear_selection_state(
    commands: &mut Commands,
    selection: &mut Selection,
    selected_pieces: &Query<Entity, With<SelectedPiece>>,
) {
    reset_selected_markers(commands, selected_pieces);
    selection.clear();
    debug!("[INPUT] Selection cleared");
}

/// Helper to find a piece entity at a specific board coordinate
fn find_piece_on_square(
    pieces: &Query<(Entity, &Piece, &HasMoved, &Transform)>,
    position: (u8, u8),
) -> Option<(Entity, Piece)> {
    pieces
        .iter()
        .find(|(_, piece, _, _)| piece.x == position.0 && piece.y == position.1)
        .map(|(entity, piece, _, _)| (entity, *piece))
}

// === Observers ===

/// Handle click on a piece
pub fn on_piece_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    mut selection: ResMut<Selection>,
    current_turn: Res<CurrentTurn>,
    // Use ParamSet to handle conflicting queries if we need mutable access for move execution
    // But `execute_move` needs `Query<(Entity, &mut Piece, &mut HasMoved)>`.
    // We only have `all_pieces` (read-only) here usually.
    // However, since we're rewriting, we can request the right queries.
    mut pieces: ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>, // For execute_move
        Query<(Entity, &Piece, &HasMoved, &Transform)>, // For finding pieces / reading state
    )>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
    game_over: Res<GameOverState>,
    mut engine: ResMut<ChessEngine>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut move_history: ResMut<MoveHistory>,
    mut captured_pieces: ResMut<CapturedPieces>,
    game_sounds: Option<Res<GameSounds>>,
) {
    if !is_primary(click.event.button) {
        return;
    }

    if game_over.is_game_over() {
        return;
    }

    let entity = click.entity;

    // We need to read piece data first. Use p1() (read-only).
    let piece_data = {
        let q = pieces.p1();
        if let Ok((_, piece, _, _)) = q.get(entity) {
            Some(*piece)
        } else {
            None
        }
    };

    let Some(clicked_piece) = piece_data else {
        warn!("[INPUT] Clicked entity {:?} has no Piece component", entity);
        return;
    };

    debug!(
        "[INPUT] Clicked piece: {:?} {:?} at ({}, {})",
        clicked_piece.color, clicked_piece.piece_type, clicked_piece.x, clicked_piece.y
    );

    // Case 1: Select our own piece
    if clicked_piece.color == current_turn.color {
        // If already selected, deselect
        if selection.selected_entity == Some(entity) {
            clear_selection_state(&mut commands, &mut selection, &selected_pieces);
            return;
        }

        // Select new piece
        // 1. Clear old
        clear_selection_state(&mut commands, &mut selection, &selected_pieces);

        // 2. Sync engine to ensure valid moves are accurate
        engine.sync_ecs_to_engine_with_transform(&pieces.p1(), &current_turn);

        // 3. Get legal moves
        let legal_moves = engine
            .get_legal_moves_for_square((clicked_piece.x, clicked_piece.y), clicked_piece.color);

        // 4. Update selection resource
        selection.selected_entity = Some(entity);
        selection.selected_position = Some((clicked_piece.x, clicked_piece.y));
        selection.possible_moves = legal_moves;

        // 5. Add visual marker
        commands.entity(entity).insert(SelectedPiece {
            entity,
            position: (clicked_piece.x, clicked_piece.y),
        });

        debug!(
            "[INPUT] Selected {:?} at ({}, {})",
            clicked_piece.piece_type, clicked_piece.x, clicked_piece.y
        );
        return;
    }

    // Case 2: Clicked enemy piece (Potential Capture)
    if selection.is_selected() {
        if let Some(selected_entity) = selection.selected_entity {
            // Validate if this is a legal move
            let target_pos = (clicked_piece.x, clicked_piece.y);
            if selection.possible_moves.contains(&target_pos) {
                // Execute Capture
                let move_sound = game_sounds.as_ref().map(|s| s.move_piece.clone());
                let capture_sound = game_sounds.as_ref().map(|s| s.capture_piece.clone());

                // Find the selected piece data from the query
                // We need to fetch it before calling execute_move because pieces.p0() will be borrowed uniquely
                let (selected_piece_data, _has_moved) = {
                    let q = pieces.p1();
                    if let Ok((_, p, hm, _)) = q.get(selected_entity) {
                        (*p, hm.moved)
                    } else {
                        // Should not happen if selection state is valid
                        warn!("[INPUT] Selected entity not found in query");
                        return;
                    }
                };

                let was_first_move = !_has_moved;

                // Capture info
                let capture_info = Some(CapturedTarget {
                    entity: entity, // The clicked enemy piece is the target
                    piece_type: clicked_piece.piece_type,
                    color: clicked_piece.color,
                });

                // EXECUTE
                let success = execute_move(
                    "piece_click_capture",
                    &mut commands,
                    selected_entity,
                    selected_piece_data,
                    target_pos,
                    capture_info,
                    was_first_move,
                    &mut pending_turn,
                    &mut move_history,
                    &mut captured_pieces,
                    &mut engine,
                    &mut pieces.p0(), // Mutable access
                    move_sound,
                    capture_sound,
                );

                if success {
                    clear_selection_state(&mut commands, &mut selection, &selected_pieces);
                }
            } else {
                // Invalid capture attempt -> Deselect
                debug!("[INPUT] Invalid capture attempt");
                clear_selection_state(&mut commands, &mut selection, &selected_pieces);
            }
        }
    }
}

/// Handle click on a square
pub fn on_square_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    mut selection: ResMut<Selection>,
    square_query: Query<&Square>,
    // Same paramset pattern for pieces
    mut pieces: ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>, // For execute_move
        Query<(Entity, &Piece, &HasMoved, &Transform)>, // For finding pieces
    )>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
    game_over: Res<GameOverState>,
    mut engine: ResMut<ChessEngine>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut move_history: ResMut<MoveHistory>,
    mut captured_pieces: ResMut<CapturedPieces>,
    game_sounds: Option<Res<GameSounds>>,
    current_turn: Res<CurrentTurn>, // Add current turn for piece selection via square
) {
    if !is_primary(click.event.button) {
        return;
    }

    let Ok(square) = square_query.get(click.entity) else {
        return;
    };

    let target_pos = (square.x, square.y);
    debug!("[INPUT] Clicked square at ({}, {})", square.x, square.y);

    // Check if there is a piece on this square (to delegate to selection logic if needed)
    // Note: If the user clicks the square *under* a piece, picking often hits the piece first.
    // But if they hit the corner of the square, or if piece picking is disabled, we might hit square.
    // Also useful for empty squares.

    // Check occupancy
    let occupant = {
        let q = pieces.p1();
        find_piece_on_square(&q, target_pos)
    };

    if let Some((piece_entity, piece)) = occupant {
        // Delegate to piece selection logic if it's our piece
        if piece.color == current_turn.color {
            // Logic similar to on_piece_click's selection
            // ... duplicate logic or simple re-route?
            // Since we can't easily call on_piece_click due to different input types, copy logic.

            // If already selected, deselect
            if selection.selected_entity == Some(piece_entity) {
                clear_selection_state(&mut commands, &mut selection, &selected_pieces);
                return;
            }

            // Select
            clear_selection_state(&mut commands, &mut selection, &selected_pieces);
            engine.sync_ecs_to_engine_with_transform(&pieces.p1(), &current_turn);
            let legal_moves = engine.get_legal_moves_for_square((piece.x, piece.y), piece.color);
            selection.selected_entity = Some(piece_entity);
            selection.selected_position = Some((piece.x, piece.y));
            selection.possible_moves = legal_moves;
            commands.entity(piece_entity).insert(SelectedPiece {
                entity: piece_entity,
                position: (piece.x, piece.y),
            });
            debug!(
                "[INPUT] Selected via square {:?} at ({}, {})",
                piece.piece_type, piece.x, piece.y
            );
            return;
        }
        // If enemy piece, fall through to move logic (capture)
    }

    // Move Logic
    if selection.is_selected() {
        if let Some(selected_entity) = selection.selected_entity {
            if selection.possible_moves.contains(&target_pos) {
                // Execute Move
                let move_sound = game_sounds.as_ref().map(|s| s.move_piece.clone());
                let capture_sound = game_sounds.as_ref().map(|s| s.capture_piece.clone());

                let (selected_piece_data, _has_moved) = {
                    let q = pieces.p1();
                    if let Ok((_, p, hm, _)) = q.get(selected_entity) {
                        (*p, hm.moved)
                    } else {
                        return;
                    }
                };
                let was_first_move = !_has_moved;

                // Check capture (from square click, we assume we checked occupant above)
                // If occupant exists and is enemy, it's a capture.
                let capture_info = if let Some((occ_entity, occ_piece)) = occupant {
                    Some(CapturedTarget {
                        entity: occ_entity,
                        piece_type: occ_piece.piece_type,
                        color: occ_piece.color,
                    })
                } else {
                    None
                };

                let success = execute_move(
                    "square_click_move",
                    &mut commands,
                    selected_entity,
                    selected_piece_data,
                    target_pos,
                    capture_info,
                    was_first_move,
                    &mut pending_turn,
                    &mut move_history,
                    &mut captured_pieces,
                    &mut engine,
                    &mut pieces.p0(),
                    move_sound,
                    capture_sound,
                );

                if success {
                    clear_selection_state(&mut commands, &mut selection, &selected_pieces);
                }
            } else {
                // Clicked empty square that is not a valid move -> Deselect
                clear_selection_state(&mut commands, &mut selection, &selected_pieces);
            }
        }
    }
}

// === Stubs to satisfy imports if needed ===

pub fn on_piece_drag_start(_: On<Pointer<DragStart>>) {}
pub fn on_piece_drag(_: On<Pointer<Drag>>) {}
pub fn on_piece_drag_end(_: On<Pointer<DragEnd>>) {}
