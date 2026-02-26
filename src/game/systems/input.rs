//! Input handling system for chess game interaction
//!
//! This module handles user input events (clicks, drags) for interacting with the chess board.
//! It uses Bevy's observer pattern for entity-specific event handling.
//!
//! # Architecture
//!
//! - **Observers**: `on_piece_click` and `on_square_click` capture pointer events.
//! - **System Params**: `InputSystemParams` groups common resources/queries to reduce argument bloat.
//! - **Helper Logic**: `try_select_piece` and `try_move_sequence` handle the core game state updates.
//!
//! # Selection Logic
//!
//! 1. Verify click is primary button (left click).
//! 2. If valid locally owned piece -> Select it.
//! 3. If valid target square/piece -> Attempt move.
//! 4. If invalid -> Clear selection.

use crate::engine::board_state::ChessEngine;
use crate::game::components::{HasMoved, SelectedPiece};
use crate::game::resources::{
    CapturedPieces, CurrentTurn, GameOverState, GameSounds, MoveHistory, PendingTurnAdvance,
    Selection,
};
use crate::game::systems::shared::{execute_move, find_piece_on_square, CapturedTarget, MoveContext};
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use bevy::ecs::system::SystemParam;
use bevy::picking::events::{Click, Drag, DragEnd, DragStart, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
// use bevy::ecs::event::EventWriter; // EventWriter is in prelude

/// Query for mutable access to pieces (used for executing moves)
pub type PieceMutQuery<'w, 's> = Query<'w, 's, (Entity, &'static mut Piece, &'static mut HasMoved)>;

/// Query for read-only access to pieces (used for validation/selection)
pub type PieceReadOnlyQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Piece,
        &'static HasMoved,
        &'static Transform,
    ),
>;

/// Grouped system parameters for input handling to reduce argument count
#[derive(SystemParam)]
pub struct InputSystemParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub selection: ResMut<'w, Selection>,
    pub current_turn: Res<'w, CurrentTurn>,
    pub pieces: ParamSet<'w, 's, (PieceMutQuery<'w, 's>, PieceReadOnlyQuery<'w, 's>)>,
    pub selected_pieces: Query<'w, 's, Entity, With<SelectedPiece>>,
    pub game_over: Res<'w, GameOverState>,
    pub engine: ResMut<'w, ChessEngine>,
    pub pending_turn: ResMut<'w, PendingTurnAdvance>,
    pub move_history: ResMut<'w, MoveHistory>,
    pub captured_pieces: ResMut<'w, CapturedPieces>,
    pub game_sounds: Option<Res<'w, GameSounds>>,
    pub move_events: MessageWriter<'w, crate::game::events::MoveMadeEvent>,
}

// Helper alias for Option<Res> if needed, or just use Option<Res>
// ResWithStandard is not a thing. Just Option<Res<'w, BraidClientResource>>.
// Bevy SystemParam macro handles Option<Res<T>>.

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

// === Helpers ===

/// Attempts to select a piece
///
/// Validates ownership (current turn) and updates selection state.
/// Also calculates legal moves for the selected piece.
fn try_select_piece(
    params: &mut InputSystemParams,
    entity: Entity,
    piece: Piece,
    is_square_click: bool,
) {
    // If already selected, deselect
    if params.selection.selected_entity == Some(entity) {
        clear_selection_state(
            &mut params.commands,
            &mut params.selection,
            &params.selected_pieces,
        );
        return;
    }

    // Select new piece
    clear_selection_state(
        &mut params.commands,
        &mut params.selection,
        &params.selected_pieces,
    );
    params
        .engine
        .sync_ecs_to_engine_with_transform(&params.pieces.p1(), &params.current_turn);

    let legal_moves = params
        .engine
        .get_legal_moves_for_square((piece.x, piece.y), piece.color);
    params.selection.selected_entity = Some(entity);
    params.selection.selected_position = Some((piece.x, piece.y));
    params.selection.possible_moves = legal_moves;
    params.commands.entity(entity).insert(SelectedPiece {
        entity,
        position: (piece.x, piece.y),
    });

    if is_square_click {
        debug!(
            "[INPUT] Selected via square {:?} at ({}, {})",
            piece.piece_type, piece.x, piece.y
        );
    } else {
        debug!(
            "[INPUT] Selected {:?} at ({}, {})",
            piece.piece_type, piece.x, piece.y
        );
    }
}

/// Attempts to execute a move sequence
///
/// Validates move legality, handles multiplayer communication,
/// and executes the move via `execute_move`.
fn try_move_sequence(
    params: &mut InputSystemParams,
    target_pos: (u8, u8),
    capture_info: Option<CapturedTarget>,
    context_name: &str,
) {
    if !params.selection.is_selected() {
        return;
    }
    let Some(selected_entity) = params.selection.selected_entity else {
        return;
    };

    if !params.selection.possible_moves.contains(&target_pos) {
        if context_name == "piece_click_capture" {
            debug!("[INPUT] Invalid capture attempt");
        }
        clear_selection_state(
            &mut params.commands,
            &mut params.selection,
            &params.selected_pieces,
        );
        return;
    }

    let move_sound = params.game_sounds.as_ref().map(|s| s.move_piece.clone());
    let capture_sound = params.game_sounds.as_ref().map(|s| s.capture_piece.clone());

    let (selected_piece_data, was_first_move) = {
        let q = params.pieces.p1();
        if let Ok((_, p, hm, _)) = q.get(selected_entity) {
            (*p, !hm.moved)
        } else {
            warn!("[INPUT] Selected piece not found query");
            return;
        }
    };

    // Multiplayer Interception removed for Pure Braid Transition

    let ctx = MoveContext {
        origin: context_name,
        entity: selected_entity,
        piece: selected_piece_data,
        target: target_pos,
        capture: capture_info,
        promotion: None,
        was_first_move,
        remote: false,
        move_sound,
        capture_sound,
    };

    let success = execute_move(
        &ctx,
        &mut params.commands,
        &mut params.pending_turn,
        &mut params.move_history,
        &mut params.captured_pieces,
        &mut params.engine,
        &mut params.pieces.p0(),
        Some(&mut params.move_events),
    );

    if success {
        clear_selection_state(
            &mut params.commands,
            &mut params.selection,
            &params.selected_pieces,
        );
    }
}

// === Observers ===

/// Observer system: Handle click on a piece
///
/// Triggers piece selection or capture attempt.
pub fn on_piece_click(click: On<Pointer<Click>>, mut params: InputSystemParams) {
    if !is_primary(click.event.button) {
        return;
    }

    if params.game_over.is_game_over() {
        return;
    }

    // Skip click handling if we're currently dragging (drag_end will handle it)
    if params.selection.is_dragging {
        return;
    }

    let entity = click.entity;

    let piece_data = {
        let q = params.pieces.p1();
        if let Ok((_, piece, _, _)) = q.get(entity) {
            Some(*piece)
        } else {
            None
        }
    };

    let Some(clicked_piece) = piece_data else {
        debug!(
            "[INPUT] Clicked entity {:?} has no Piece component – ignoring",
            entity
        );
        return;
    };

    debug!(
        "[INPUT] Clicked piece: {:?} {:?} at ({}, {})",
        clicked_piece.color, clicked_piece.piece_type, clicked_piece.x, clicked_piece.y
    );

    // Case 1: Clicked our own piece -> Select
    if clicked_piece.color == params.current_turn.color {
        try_select_piece(&mut params, entity, clicked_piece, false);
        return;
    }

    // Case 2: Clicked enemy piece -> Capture
    let target_pos = (clicked_piece.x, clicked_piece.y);
    let capture_info = Some(CapturedTarget {
        entity,
        piece_type: clicked_piece.piece_type,
        color: clicked_piece.color,
    });

    try_move_sequence(&mut params, target_pos, capture_info, "piece_click_capture");
}

/// Observer system: Handle drag start on a piece
///
/// Initiates drag-and-drop by selecting the piece and marking it as dragging.
pub fn on_piece_drag_start(
    drag_start: On<Pointer<DragStart>>,
    mut params: InputSystemParams,
) {
    if params.game_over.is_game_over() {
        return;
    }

    let entity = drag_start.entity;

    // Get the piece data
    let piece_data = {
        let q = params.pieces.p1();
        if let Ok((_, piece, _, _)) = q.get(entity) {
            Some(*piece)
        } else {
            None
        }
    };

    let Some(piece) = piece_data else {
        return;
    };

    // Only allow dragging our own pieces
    if piece.color != params.current_turn.color {
        return;
    }

    // Select the piece and mark as dragging
    try_select_piece(&mut params, entity, piece, false);
    params.selection.begin_drag();

    debug!("[INPUT] Started dragging piece at ({}, {})", piece.x, piece.y);
}

/// Observer system: Handle drag on a piece
///
/// Currently just tracks that dragging is in progress. Visual feedback
/// could be added here (e.g., lifting the piece, showing ghost).
pub fn on_piece_drag(_: On<Pointer<Drag>>, _params: InputSystemParams) {
    // Dragging is handled by the selection state
    // Visual feedback (like lifting the piece) could be added here
}

/// Observer system: Handle drag end on a piece
///
/// Attempts to execute a move to the square where the piece was dropped.
pub fn on_piece_drag_end(
    drag_end: On<Pointer<DragEnd>>,
    mut params: InputSystemParams,
    square_query: Query<(Entity, &Square, &Transform)>,
    piece_query: Query<(Entity, &Piece, &Transform)>,
) {
    if !params.selection.is_dragging {
        return;
    }

    params.selection.end_drag();

    // Get the piece that was dragged
    let dragged_entity = drag_end.entity;
    let dragged_piece = piece_query.get(dragged_entity).ok();

    // Find which square the piece was dropped on by checking the piece's current position
    // or finding the square under the pointer
    let target_square = if let Ok((_, _, transform)) = piece_query.get(dragged_entity) {
        // Calculate board position from world position
        let world_pos = transform.translation;
        let file = world_pos.x.round() as i32;
        let rank = world_pos.z.round() as i32;

        // Find square at this board position
        square_query.iter().find(|(_, square, _)| {
            square.x as i32 == file && square.y as i32 == rank
        })
    } else {
        None
    };

    if let Some((_, square, _)) = target_square {
        let target_pos = (square.x, square.y);
        debug!("[INPUT] Dropped piece on square ({}, {})", square.x, square.y);

        // Check if there's a piece on this square (capture)
        let occupant = {
            let q = params.pieces.p1();
            find_piece_on_square(&q, target_pos)
        };

        let capture_info = occupant.map(|(e, p)| CapturedTarget {
            entity: e,
            piece_type: p.piece_type,
            color: p.color,
        });

        try_move_sequence(&mut params, target_pos, capture_info, "drag_drop");
    } else {
        // Dropped on invalid location - cancel drag
        debug!("[INPUT] Dropped on invalid location - cancelling drag");
        clear_selection_state(
            &mut params.commands,
            &mut params.selection,
            &params.selected_pieces,
        );
    }
}

/// Observer system: Handle click on a square
///
/// Triggers move to empty square or selection of piece on that square.
pub fn on_square_click(
    click: On<Pointer<Click>>,
    mut params: InputSystemParams,
    square_query: Query<&Square>,
) {
    if !is_primary(click.event.button) {
        return;
    }
    if params.game_over.is_game_over() {
        return;
    }

    // Skip if we're dragging - drag_end handles the drop
    if params.selection.is_dragging {
        return;
    }

    let Ok(square) = square_query.get(click.entity) else {
        return;
    };

    let target_pos = (square.x, square.y);
    debug!("[INPUT] Clicked square at ({}, {})", square.x, square.y);

    let occupant = {
        let q = params.pieces.p1();
        find_piece_on_square(&q, target_pos)
    };

    if let Some((piece_entity, piece)) = occupant {
        if piece.color == params.current_turn.color {
            try_select_piece(&mut params, piece_entity, piece, true);
            return;
        }
    }

    let capture_info = occupant.map(|(e, p)| CapturedTarget {
        entity: e,
        piece_type: p.piece_type,
        color: p.color,
    });

    try_move_sequence(&mut params, target_pos, capture_info, "square_click_move");
}

/// Observer system: Handle hover on a piece
pub fn on_piece_hover(_: On<Pointer<DragStart>>) {
    // Could add hover visual feedback here
}

/// Observer system: Handle unhover on a piece
pub fn on_piece_unhover(_: On<Pointer<DragStart>>) {
    // Could remove hover visual feedback here
}
