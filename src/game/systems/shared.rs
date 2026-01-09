use crate::game::components::{FadingCapture, HasMoved, MoveRecord, PieceMoveAnimation};
use crate::game::resources::{CapturedPieces, ChessEngine, MoveHistory, PendingTurnAdvance};
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::audio::{AudioPlayer, AudioSource};
use bevy::prelude::*;
use chess_engine::do_move;

/// Data required to identify a captured piece target
#[derive(Clone, Copy, Debug)]
pub struct CapturedTarget {
    pub entity: Entity,
    pub piece_type: PieceType,
    pub color: PieceColor,
}

/// Helper to handle audio playback for moves
pub fn play_move_audio(
    commands: &mut Commands,
    move_sound: Option<Handle<AudioSource>>,
    capture_happened: bool,
) {
    if capture_happened {
        // Capture sound handled in trigger_capture/apply_capture usually?
        // In input.rs, play_move_audio respects capture_happened.
        // It plays move_sound ONLY IF no capture happened (because capture sound is played separately).
        if let Some(_sound) = move_sound {
            // commands.spawn(AudioPlayer::new(sound)); // This was playing move sound even on capture?
            // In input.rs: `if capture_happened { return; }`
            return;
        }
    }
    // Only play move sound if NOT a capture
    if let Some(sound) = move_sound {
        commands.spawn(AudioPlayer::new(sound));
    }
}

/// Apply visual and logical state for a captured piece
/// Now uses fading animation instead of instant move
pub fn apply_capture(
    commands: &mut Commands,
    captured_pieces: &mut CapturedPieces,
    capture_sound: Option<Handle<AudioSource>>,
    target: CapturedTarget,
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

    // Add FadingCapture component for fade-out animation instead of instant move
    commands.entity(target.entity).insert(FadingCapture {
        timer: Timer::from_seconds(0.5, TimerMode::Once),
        capture_zone_pos: capture_pos,
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
    commands: &mut Commands,
    pieces: &mut Query<(Entity, &mut Piece, &mut HasMoved)>,
    move_history: &mut MoveHistory,
) -> bool {
    let Ok((_, mut piece_component, mut has_moved)) = pieces.get_mut(entity) else {
        error!("[SHARED] {origin}: failed to access piece after move");
        return false;
    };
    let move_record = MoveRecord {
        piece_type: piece_component.piece_type,
        piece_color: piece_component.color,
        from: from_pos,
        to: target,
        captured: capture.map(|data| data.piece_type),
        is_castling: false, // AI/Input logic currently simplifies this (handled by engine state implicit?)
        is_en_passant: false,
        is_check: false,
        is_checkmate: false,
    };
    move_history.add_move(move_record);
    piece_component.x = target.0;
    piece_component.y = target.1;
    commands.entity(entity).insert(PieceMoveAnimation::new(
        Vec3::new(from_pos.0 as f32, 0.0, from_pos.1 as f32),
        Vec3::new(target.0 as f32, 0.0, target.1 as f32),
        0.25,
    ));

    has_moved.moved = true;
    has_moved.move_count += 1;
    true
}

/// Core function to execute a validated move
///
/// Handles:
/// 1. Engine update
/// 2. Capture processing (visuals + sound)
/// 3. Move sound (if no capture)
/// 4. ECS piece state update (coords, history, animation)
/// 5. Engine -> ECS sync
/// 6. Turn advance request
#[allow(clippy::too_many_arguments)]
pub fn execute_move(
    origin: &str,
    commands: &mut Commands,
    // Move parameters
    entity: Entity,
    piece: Piece,
    target: (u8, u8),
    capture: Option<CapturedTarget>,
    was_first_move: bool,
    // Resources
    pending_turn: &mut PendingTurnAdvance,
    move_history: &mut MoveHistory,
    captured_pieces: &mut CapturedPieces,
    engine: &mut ChessEngine,
    // Queries
    pieces_query: &mut Query<(Entity, &mut Piece, &mut HasMoved)>,
    // Sounds
    move_sound: Option<Handle<AudioSource>>,
    capture_sound: Option<Handle<AudioSource>>,
) -> bool {
    let from_pos = (piece.x, piece.y);
    let src_index = ChessEngine::square_to_index(from_pos.0, from_pos.1);
    let dst_index = ChessEngine::square_to_index(target.0, target.1);

    // 1. Update Engine
    do_move(&mut engine.game, src_index, dst_index, true);

    // 2. Handle Capture
    let capture_happened = if let Some(target_capture) = capture {
        apply_capture(commands, captured_pieces, capture_sound, target_capture);
        true
    } else {
        false
    };

    // 3. Play Move Sound (if not capture)
    if !capture_happened {
        if let Some(sound) = move_sound {
            commands.spawn(AudioPlayer::new(sound));
        }
    }

    // 4. Update ECS Piece
    if !update_piece_state(
        origin,
        entity,
        from_pos,
        target,
        was_first_move,
        capture,
        commands,
        pieces_query,
        move_history,
    ) {
        return false;
    }

    // 5. Sync Engine -> ECS (updates castling rights, en passant, etc.)
    engine.sync_engine_to_ecs(commands, pieces_query);

    // 6. Request Turn Advance
    pending_turn.request(piece.color);

    // Consolidated move log
    debug!(
        "[MOVE] {}{}→{}{}{}",
        (b'a' + from_pos.1) as char,
        from_pos.0 + 1,
        (b'a' + target.1) as char,
        target.0 + 1,
        if capture_happened { " (capture)" } else { "" }
    );

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
