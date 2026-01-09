use crate::game::components::{HasMoved, SelectedPiece};
use crate::game::resources::{
    CapturedPieces, ChessEngine, CurrentTurn, GameOverState, GameSounds, MoveHistory,
    PendingTurnAdvance, Selection,
};
use crate::game::systems::shared::CapturedTarget;
use crate::game::systems::shared::{execute_move, find_piece_on_square};
use crate::networking::client::MultiplayerSession;
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use bevy::picking::events::{Click, Drag, DragEnd, DragStart, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use lightyear::prelude::*;
use shared::protocol::{Channel1, GameMessage};

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

#[allow(clippy::too_many_arguments)]
fn try_select_piece(
    commands: &mut Commands,
    selection: &mut Selection,
    selected_pieces: &Query<Entity, With<SelectedPiece>>,
    engine: &mut ChessEngine,
    current_turn: &CurrentTurn,
    pieces_query: &Query<(Entity, &Piece, &HasMoved, &Transform)>,
    entity: Entity,
    piece: Piece,
    is_square_click: bool,
) {
    // If already selected, deselect
    if selection.selected_entity == Some(entity) {
        clear_selection_state(commands, selection, selected_pieces);
        return;
    }

    // Select new piece
    clear_selection_state(commands, selection, selected_pieces);
    engine.sync_ecs_to_engine_with_transform(pieces_query, current_turn);

    let legal_moves = engine.get_legal_moves_for_square((piece.x, piece.y), piece.color);

    selection.selected_entity = Some(entity);
    selection.selected_position = Some((piece.x, piece.y));
    selection.possible_moves = legal_moves;

    commands.entity(entity).insert(SelectedPiece {
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

#[allow(clippy::too_many_arguments)]
fn try_move_sequence(
    commands: &mut Commands,
    selection: &mut Selection,
    selected_pieces: &Query<Entity, With<SelectedPiece>>,
    pieces: &mut ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>,
        Query<(Entity, &Piece, &HasMoved, &Transform)>,
    )>,
    engine: &mut ChessEngine,
    pending_turn: &mut PendingTurnAdvance,
    move_history: &mut MoveHistory,
    captured_pieces: &mut CapturedPieces,
    game_sounds: &Option<Res<GameSounds>>,
    target_pos: (u8, u8),
    capture_info: Option<CapturedTarget>,
    context_name: &str,
    multiplayer: &Res<MultiplayerSession>,
    sender: &mut Option<Mut<MessageSender<GameMessage>>>,
) {
    if !selection.is_selected() {
        return;
    }
    let Some(selected_entity) = selection.selected_entity else {
        return;
    };

    if !selection.possible_moves.contains(&target_pos) {
        if context_name == "piece_click_capture" {
            debug!("[INPUT] Invalid capture attempt");
        }
        clear_selection_state(commands, selection, selected_pieces);
        return;
    }

    let move_sound = game_sounds.as_ref().map(|s| s.move_piece.clone());
    let capture_sound = game_sounds.as_ref().map(|s| s.capture_piece.clone());

    let (selected_piece_data, was_first_move) = {
        let q = pieces.p1();
        if let Ok((_, p, hm, _)) = q.get(selected_entity) {
            (*p, !hm.moved)
        } else {
            warn!("[INPUT] Selected piece not found query");
            return;
        }
    };

    // Multiplayer Interception
    if multiplayer.is_active {
        if let Some(sender) = sender {
            // Check turn? Protocol usually handles validation, but client-side check is good UX.
            // For now, simpler to just send.
            info!(
                "[MULTIPLAYER] Sending Move: ({}, {}) -> ({}, {})",
                selected_piece_data.x, selected_piece_data.y, target_pos.0, target_pos.1
            );
            let _ = sender.send::<Channel1>(GameMessage::SubmitMove {
                from: (selected_piece_data.x, selected_piece_data.y),
                to: target_pos,
            });
            clear_selection_state(commands, selection, selected_pieces);
            return;
        } else {
            warn!("[MULTIPLAYER] MessageSender not available!");
        }
    }

    let success = execute_move(
        context_name,
        commands,
        selected_entity,
        selected_piece_data,
        target_pos,
        capture_info,
        was_first_move,
        pending_turn,
        move_history,
        captured_pieces,
        engine,
        &mut pieces.p0(),
        move_sound,
        capture_sound,
    );

    if success {
        clear_selection_state(commands, selection, selected_pieces);
    }
}

// === Observers ===

/// Handle click on a piece
pub fn on_piece_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    mut selection: ResMut<Selection>,
    current_turn: Res<CurrentTurn>,
    mut pieces: ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>,
        Query<(Entity, &Piece, &HasMoved, &Transform)>,
    )>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
    game_over: Res<GameOverState>,
    mut engine: ResMut<ChessEngine>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut move_history: ResMut<MoveHistory>,
    mut captured_pieces: ResMut<CapturedPieces>,
    game_sounds: Option<Res<GameSounds>>,
    multiplayer: Res<MultiplayerSession>,
    mut sender_query: Query<&mut MessageSender<GameMessage>, With<Client>>,
) {
    if !is_primary(click.event.button) {
        return;
    }

    if game_over.is_game_over() {
        return;
    }

    let entity = click.entity;

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

    // Case 1: Clicked our own piece -> Select
    if clicked_piece.color == current_turn.color {
        try_select_piece(
            &mut commands,
            &mut selection,
            &selected_pieces,
            &mut engine,
            &current_turn,
            &pieces.p1(),
            entity,
            clicked_piece,
            false,
        );
        return;
    }

    // Case 2: Clicked enemy piece -> Capture
    let target_pos = (clicked_piece.x, clicked_piece.y);
    let capture_info = Some(CapturedTarget {
        entity,
        piece_type: clicked_piece.piece_type,
        color: clicked_piece.color,
    });

    try_move_sequence(
        &mut commands,
        &mut selection,
        &selected_pieces,
        &mut pieces,
        &mut engine,
        &mut pending_turn,
        &mut move_history,
        &mut captured_pieces,
        &game_sounds,
        target_pos,
        capture_info,
        "piece_click_capture",
        &multiplayer,
        &mut if let Some(client_entity) = multiplayer.client_entity {
            sender_query.get_mut(client_entity).ok()
        } else {
            None
        },
    );
}

/// Handle click on a square
pub fn on_square_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    mut selection: ResMut<Selection>,
    square_query: Query<&Square>,
    mut pieces: ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>,
        Query<(Entity, &Piece, &HasMoved, &Transform)>,
    )>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
    game_over: Res<GameOverState>,
    mut engine: ResMut<ChessEngine>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut move_history: ResMut<MoveHistory>,
    mut captured_pieces: ResMut<CapturedPieces>,
    game_sounds: Option<Res<GameSounds>>,
    current_turn: Res<CurrentTurn>,
    multiplayer: Res<MultiplayerSession>,
    mut sender_query: Query<&mut MessageSender<GameMessage>, With<Client>>,
) {
    if !is_primary(click.event.button) {
        return;
    }
    if game_over.is_game_over() {
        return;
    }

    let Ok(square) = square_query.get(click.entity) else {
        return;
    };

    let target_pos = (square.x, square.y);
    debug!("[INPUT] Clicked square at ({}, {})", square.x, square.y);

    let occupant = {
        let q = pieces.p1();
        find_piece_on_square(&q, target_pos)
    };

    if let Some((piece_entity, piece)) = occupant {
        if piece.color == current_turn.color {
            try_select_piece(
                &mut commands,
                &mut selection,
                &selected_pieces,
                &mut engine,
                &current_turn,
                &pieces.p1(),
                piece_entity,
                piece,
                true,
            );
            return;
        }
    }

    let capture_info = occupant.map(|(e, p)| CapturedTarget {
        entity: e,
        piece_type: p.piece_type,
        color: p.color,
    });

    try_move_sequence(
        &mut commands,
        &mut selection,
        &selected_pieces,
        &mut pieces,
        &mut engine,
        &mut pending_turn,
        &mut move_history,
        &mut captured_pieces,
        &game_sounds,
        target_pos,
        capture_info,
        "square_click_move",
        &multiplayer,
        &mut if let Some(client_entity) = multiplayer.client_entity {
            sender_query.get_mut(client_entity).ok()
        } else {
            None
        },
    );
}

// === Stubs to satisfy imports if needed ===

pub fn on_piece_drag_start(_: On<Pointer<DragStart>>) {}
pub fn on_piece_drag(_: On<Pointer<Drag>>) {}
pub fn on_piece_drag_end(_: On<Pointer<DragEnd>>) {}
