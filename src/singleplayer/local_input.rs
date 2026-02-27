//! Local input handling for singleplayer games.

use crate::engine::board_state::ChessEngine;
use crate::game::components::Piece;
use crate::game::components::{HasMoved, SelectedPiece};
use crate::game::resources::{
    CapturedPieces, CurrentTurn, GameOverState, GameSounds, MoveHistory, PendingTurnAdvance,
    Selection,
};
use crate::game::systems::shared::{
    execute_move, find_piece_on_square, CapturedTarget, MoveContext,
};
use crate::rendering::utils::Square;
use bevy::ecs::system::SystemParam;
use bevy::picking::events::{Click, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

/// Grouped system parameters for local input handling.
#[derive(SystemParam)]
pub struct LocalInputSystemParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub selection: ResMut<'w, Selection>,
    pub current_turn: Res<'w, CurrentTurn>,
    pub pieces: ParamSet<
        'w,
        's,
        (
            Query<'w, 's, (Entity, &'static mut Piece, &'static mut HasMoved)>,
            Query<
                'w,
                's,
                (
                    Entity,
                    &'static Piece,
                    &'static HasMoved,
                    &'static Transform,
                ),
            >,
        ),
    >,
    pub selected_pieces: Query<'w, 's, Entity, With<SelectedPiece>>,
    pub game_over: Res<'w, GameOverState>,
    pub engine: ResMut<'w, ChessEngine>,
    pub pending_turn: ResMut<'w, PendingTurnAdvance>,
    pub move_history: ResMut<'w, MoveHistory>,
    pub captured_pieces: ResMut<'w, CapturedPieces>,
    pub game_sounds: Option<Res<'w, GameSounds>>,
    pub move_events: MessageWriter<'w, crate::game::events::MoveMadeEvent>,
}

/// Helper to check if primary button (left click) was used
fn is_primary(button: PointerButton) -> bool {
    matches!(button, PointerButton::Primary)
}

/// Helper to clear selection state
fn clear_selection_state(
    commands: &mut Commands,
    selection: &mut Selection,
    selected_pieces: &Query<Entity, With<SelectedPiece>>,
) {
    for entity in selected_pieces.iter() {
        commands.entity(entity).remove::<SelectedPiece>();
    }
    selection.clear();
}

/// Attempts to select a piece
fn try_select_piece(params: &mut LocalInputSystemParams, entity: Entity, piece: Piece) {
    if params.selection.selected_entity == Some(entity) {
        clear_selection_state(
            &mut params.commands,
            &mut params.selection,
            &params.selected_pieces,
        );
        return;
    }

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
}

/// Attempts to execute a move
fn try_move_sequence(
    params: &mut LocalInputSystemParams,
    target_pos: (u8, u8),
    capture_info: Option<CapturedTarget>,
) {
    if !params.selection.is_selected() {
        return;
    }
    let Some(selected_entity) = params.selection.selected_entity else {
        return;
    };

    if !params.selection.possible_moves.contains(&target_pos) {
        clear_selection_state(
            &mut params.commands,
            &mut params.selection,
            &params.selected_pieces,
        );
        return;
    }

    let (selected_piece_data, was_first_move) = {
        let q = params.pieces.p1();
        if let Ok((_, p, hm, _)) = q.get(selected_entity) {
            (*p, !hm.moved)
        } else {
            return;
        }
    };

    let ctx = MoveContext {
        origin: "local_input",
        entity: selected_entity,
        piece: selected_piece_data,
        target: target_pos,
        capture: capture_info,
        promotion: None,
        was_first_move,
        remote: false,
        move_sound: params.game_sounds.as_ref().map(|s| s.move_piece.clone()),
        capture_sound: params.game_sounds.as_ref().map(|s| s.capture_piece.clone()),
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
        None, // BoardStateSync not available in this context
    );

    if success {
        clear_selection_state(
            &mut params.commands,
            &mut params.selection,
            &params.selected_pieces,
        );
    }
}

pub fn on_piece_click(click: On<Pointer<Click>>, mut params: LocalInputSystemParams) {
    if !is_primary(click.event.button) || params.game_over.is_game_over() {
        return;
    }

    let entity = click.entity;
    let piece_data = params.pieces.p1().get(entity).map(|(_, p, _, _)| *p).ok();

    let Some(clicked_piece) = piece_data else {
        return;
    };

    if clicked_piece.color == params.current_turn.color {
        try_select_piece(&mut params, entity, clicked_piece);
    } else {
        let target_pos = (clicked_piece.x, clicked_piece.y);
        let capture_info = Some(CapturedTarget {
            entity,
            piece_type: clicked_piece.piece_type,
            color: clicked_piece.color,
        });
        try_move_sequence(&mut params, target_pos, capture_info);
    }
}

pub fn on_square_click(
    click: On<Pointer<Click>>,
    mut params: LocalInputSystemParams,
    square_query: Query<&Square>,
) {
    if !is_primary(click.event.button) || params.game_over.is_game_over() {
        return;
    }

    let Ok(square) = square_query.get(click.entity) else {
        return;
    };
    let target_pos = (square.x, square.y);

    let occupant = find_piece_on_square(&params.pieces.p1(), target_pos);

    if let Some((piece_entity, piece)) = occupant {
        if piece.color == params.current_turn.color {
            try_select_piece(&mut params, piece_entity, piece);
            return;
        }
    }

    let capture_info = occupant.map(|(e, p)| CapturedTarget {
        entity: e,
        piece_type: p.piece_type,
        color: p.color,
    });

    try_move_sequence(&mut params, target_pos, capture_info);
}
