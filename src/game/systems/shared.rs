use crate::engine::board_state::ChessEngine;
use crate::game::components::{
    FadingCapture, HasMoved, MoveRecord, Piece, PieceColor, PieceMoveAnimation, PieceType,
};
use crate::game::events::MoveMadeEvent;
use crate::game::resources::{CapturedPieces, MoveHistory, PendingTurnAdvance};
use crate::game::sync::board_state::{BoardMove, BoardStateSync, ChessEngineExt};
use crate::rendering::pieces::PIECE_ON_BOARD_Y;
use bevy::audio::{AudioPlayer, AudioSource};
use bevy::prelude::*;

/// Data required to identify a captured piece target.
#[derive(Clone, Copy, Debug)]
pub struct CapturedTarget {
    pub entity: Entity,
    pub piece_type: PieceType,
    pub color: PieceColor,
}

/// Describes a single chess move — the "what" without the "how".
///
/// Groups the value-parameters that were previously passed individually
/// to [`execute_move`], making call sites easier to read and harder to
/// get wrong (no positional-argument confusion).
///
/// # Reference
///
/// - <https://stackoverflow.com/questions/40703863> (parameter object pattern)
#[derive(Clone, Debug)]
pub struct MoveContext<'a> {
    /// Label for log messages (e.g. `"ai"`, `"network_move"`, `"local_input"`).
    pub origin: &'a str,
    /// Entity being moved.
    pub entity: Entity,
    /// Snapshot of the piece component at move time.
    pub piece: Piece,
    /// Destination square `(file, rank)`.
    pub target: (u8, u8),
    /// Captured piece, if any.
    pub capture: Option<CapturedTarget>,
    /// Promotion target type, if pawn reaches last rank.
    pub promotion: Option<PieceType>,
    /// Whether this is the piece's first move (enables castling / double-pawn).
    pub was_first_move: bool,
    /// `true` when the move originated from a remote peer.
    pub remote: bool,
    /// Move sound handle (optional).
    pub move_sound: Option<Handle<AudioSource>>,
    /// Capture sound handle (optional).
    pub capture_sound: Option<Handle<AudioSource>>,
}

/// Helper to handle audio playback for moves
pub fn play_move_audio(
    commands: &mut Commands,
    move_sound: Option<Handle<AudioSource>>,
    capture_happened: bool,
) {
    if capture_happened {
        if let Some(_sound) = move_sound {
            return;
        }
    }
    // Only play move sound if NOT a capture
    if let Some(sound) = move_sound {
        commands.spawn(AudioPlayer::new(sound));
    }
}

/// Apply visual and logical state for a captured piece.
/// Now uses fading animation instead of instant move.
pub fn apply_capture(
    commands: &mut Commands,
    captured_pieces: &mut CapturedPieces,
    capture_sound: Option<Handle<AudioSource>>,
    target: CapturedTarget,
    children_query: &Query<&Children>,
    material_query: &Query<&MeshMaterial3d<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
) {
    if let Some(sound) = capture_sound {
        commands.spawn(AudioPlayer::new(sound));
    }
    let count_of_same_type = match target.color {
        PieceColor::White => captured_pieces
            .black_captured
            .iter()
            .filter(|&&piece| piece == target.piece_type)
            .count(),
        PieceColor::Black => captured_pieces
            .white_captured
            .iter()
            .filter(|&&piece| piece == target.piece_type)
            .count(),
    };
    captured_pieces.add_capture(target.color, target.piece_type);
    let capture_pos = crate::rendering::pieces::calculate_capture_position(
        target.color,
        target.piece_type,
        count_of_same_type,
    );

    // Capture the original color from the first child mesh's material
    let mut original_color = Color::WHITE; // Default fallback
    if let Ok(children) = children_query.get(target.entity) {
        for child in children.iter() {
            if let Ok(material_handle) = material_query.get(child) {
                if let Some(material) = materials.get(&material_handle.0) {
                    original_color = material.base_color;
                    break; // Use the first child's material color
                }
            }
        }
    }

    // Add FadingCapture component for fade-out animation instead of instant move
    // Longer duration (1.0s) for smoother visual effect
    commands.entity(target.entity).insert(FadingCapture {
        timer: Timer::from_seconds(1.0, TimerMode::Once),
        capture_zone_pos: capture_pos,
        original_color,
    });
}

/// Updates ECS components for a moved piece (position, history, animation)
pub fn update_piece_state(
    origin: &str,
    entity: Entity,
    from_pos: (u8, u8),
    target: (u8, u8),
    _was_first_move: bool,
    capture: Option<CapturedTarget>,
    promotion: Option<PieceType>,
    commands: &mut Commands,
    pieces: &mut Query<(Entity, &mut Piece, &mut HasMoved)>,
    move_history: &mut MoveHistory,
) -> bool {
    let Ok((_, mut piece_component, mut has_moved)) = pieces.get_mut(entity) else {
        error!("[SHARED] {origin}: failed to access piece after move");
        return false;
    };

    // Apply promotion if applicable
    if let Some(new_type) = promotion {
        debug!("[SHARED] {origin}: Promoting piece to {:?}", new_type);
        piece_component.piece_type = new_type;
    }

    let move_record = MoveRecord {
        piece_type: piece_component.piece_type,
        piece_color: piece_component.color,
        from: from_pos,
        to: target,
        captured: capture.map(|data| data.piece_type),
        is_castling: false,
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };
    move_history.add_move(move_record);
    piece_component.x = target.0;
    piece_component.y = target.1;
    // Use PIECE_ON_BOARD_Y so the animation stays on the board surface (y=0.05),
    // matching the spawn position and the snap target in animate_piece_movement.
    // Coordinate system: X = 7 - rank, Z = file (matches reference)
    commands.entity(entity).insert(PieceMoveAnimation::new(
        Vec3::new((7 - from_pos.1) as f32, PIECE_ON_BOARD_Y, from_pos.0 as f32),
        Vec3::new((7 - target.1) as f32, PIECE_ON_BOARD_Y, target.0 as f32),
        0.25,
    ));

    has_moved.moved = true;
    has_moved.move_count += 1;
    true
}

/// Core function to execute a validated move.
///
/// Accepts a [`MoveContext`] (the "what") plus mutable ECS handles (the "how").
/// This keeps the call-site readable and prevents positional-argument mistakes.
#[allow(clippy::too_many_arguments)]
pub fn execute_move(
    ctx: &MoveContext<'_>,
    commands: &mut Commands,
    pending_turn: &mut PendingTurnAdvance,
    move_history: &mut MoveHistory,
    captured_pieces: &mut CapturedPieces,
    engine: &mut ChessEngine,
    pieces_query: &mut Query<(Entity, &mut Piece, &mut HasMoved)>,
    move_events: Option<&mut MessageWriter<MoveMadeEvent>>,
    board_sync: Option<&mut BoardStateSync>,
    children_query: &Query<&Children>,
    material_query: &Query<&MeshMaterial3d<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
) -> bool {
    // 1. Play Audio
    play_move_audio(commands, ctx.move_sound.clone(), ctx.capture.is_some());

    // 2. Handle Capture
    if let Some(target_cap) = ctx.capture {
        apply_capture(
            commands,
            captured_pieces,
            ctx.capture_sound.clone(),
            target_cap,
            children_query,
            material_query,
            materials,
        );
    }

    // 3. Update Piece State
    let from_pos = (ctx.piece.x, ctx.piece.y);
    if !update_piece_state(
        ctx.origin,
        ctx.entity,
        from_pos,
        ctx.target,
        ctx.was_first_move,
        ctx.capture,
        ctx.promotion,
        commands,
        pieces_query,
        move_history,
    ) {
        return false;
    }

    // 4. Advance Turn
    pending_turn.request(ctx.piece.color);

    // 5. Update Engine State (for P2P sync and FEN export)
    update_engine_state_after_move(
        engine,
        ctx.piece.piece_type,
        ctx.piece.color,
        from_pos,
        ctx.target,
        ctx.capture.is_some(),
        ctx.was_first_move,
    );

    // 6. Broadcast Board State (for P2P sync)
    if let Some(sync) = board_sync {
        // Only broadcast local moves (not remote moves received from network)
        if !ctx.remote {
            let board_move = BoardMove {
                from: from_pos,
                to: ctx.target,
                piece_type: ctx.piece.piece_type,
                piece_color: ctx.piece.color,
                capture: ctx.capture.map(|c| c.piece_type),
                promotion: ctx.promotion,
                move_number: engine.get_move_counter(),
            };
            let serialized = sync.serialize_state(engine, captured_pieces, Some(board_move));
            sync.on_local_move(board_move, serialized);
        }
    }

    // 7. Trigger Event
    if let Some(writer) = move_events {
        writer.write(MoveMadeEvent {
            from: from_pos,
            to: ctx.target,
            player: format!("{:?}", ctx.piece.color),
            piece_type: ctx.piece.piece_type,
            captured_piece: ctx.capture.map(|c| c.piece_type),
            promotion: ctx.promotion,
            remote: ctx.remote,
            game_id: None,
        });
    }

    true
}

/// Helper to find a piece entity at a specific board coordinate
pub fn find_piece_on_square(
    pieces: &Query<(Entity, &Piece, &HasMoved, &Transform)>,
    position: (u8, u8),
) -> Option<(Entity, Piece)> {
    pieces
        .iter()
        .find(|(_, piece, _, _)| piece.x == position.0 && piece.y == position.1)
        .map(|(entity, piece, _, _)| (entity, *piece))
}

/// Update ChessEngine state after a move.
///
/// Tracks:
/// - halfmove_clock: reset on capture or pawn move
/// - fullmove_counter: increment after Black's move
/// - current_turn: flip after each move
/// - en_passant: set if pawn moved 2 squares
/// - castling_rights: update if king or rook moved
fn update_engine_state_after_move(
    engine: &mut ChessEngine,
    piece_type: PieceType,
    piece_color: PieceColor,
    from: (u8, u8),
    to: (u8, u8),
    capture: bool,
    was_first_move: bool,
) {
    // Update halfmove clock: reset on capture or pawn move, otherwise increment
    let is_pawn_move = piece_type == PieceType::Pawn;
    if capture || is_pawn_move {
        engine.halfmove_clock = 0;
    } else {
        engine.halfmove_clock += 1;
    }

    // Update fullmove counter: increment after Black's move
    if piece_color == PieceColor::Black {
        engine.fullmove_counter += 1;
    }

    // Update current turn
    engine.current_turn = match piece_color {
        PieceColor::White => PieceColor::Black,
        PieceColor::Black => PieceColor::White,
    };

    // Track en passant: set if pawn moved 2 squares
    if piece_type == PieceType::Pawn && was_first_move {
        let is_white_double_push = piece_color == PieceColor::White && from.1 == 1 && to.1 == 3;
        let is_black_double_push = piece_color == PieceColor::Black && from.1 == 6 && to.1 == 4;

        if is_white_double_push {
            // En passant target is square behind the pawn (rank 3 in 1-8 notation = index 2 in 0-7)
            engine.en_passant = Some(format!("{}{}", file_to_char(from.0), '3'));
        } else if is_black_double_push {
            // En passant target is square behind the pawn (rank 6 in 1-8 notation = index 5 in 0-7)
            engine.en_passant = Some(format!("{}{}", file_to_char(from.0), '6'));
        } else {
            engine.en_passant = None;
        }
    } else {
        engine.en_passant = None;
    }

    // Update castling rights if king or rook moved
    update_castling_rights(engine, piece_type, piece_color, from, was_first_move);
}

/// Convert file index (0-7) to character ('a'-'h')
fn file_to_char(file: u8) -> char {
    (b'a' + file) as char
}

/// Update castling rights when king or rook moves
fn update_castling_rights(
    engine: &mut ChessEngine,
    piece_type: PieceType,
    piece_color: PieceColor,
    from: (u8, u8),
    was_first_move: bool,
) {
    // Only update on first move of king or rook
    if !was_first_move {
        return;
    }

    let mut rights: Vec<char> = engine.castling_rights.chars().collect();

    match piece_type {
        PieceType::King => {
            // King moved - lose both castling rights for that color
            if piece_color == PieceColor::White {
                rights.retain(|&c| c != 'K' && c != 'Q');
            } else {
                rights.retain(|&c| c != 'k' && c != 'q');
            }
        }
        PieceType::Rook => {
            // Rook moved - lose specific castling right
            // White rooks: kingside at (7, 0), queenside at (0, 0)
            // Black rooks: kingside at (7, 7), queenside at (0, 7)
            match (piece_color, from) {
                (PieceColor::White, (7, 0)) => rights.retain(|&c| c != 'K'),
                (PieceColor::White, (0, 0)) => rights.retain(|&c| c != 'Q'),
                (PieceColor::Black, (7, 7)) => rights.retain(|&c| c != 'k'),
                (PieceColor::Black, (0, 7)) => rights.retain(|&c| c != 'q'),
                _ => {}
            }
        }
        _ => {}
    }

    if rights.is_empty() {
        engine.castling_rights = "-".to_string();
    } else {
        engine.castling_rights = rights.into_iter().collect();
    }
}
