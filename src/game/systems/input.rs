//! Observer-based input handling for piece selection and movement
//!
//! Implements Bevy 0.17's idiomatic `.observe()` pattern for pointer interactions.
//! This replaces the deprecated MessageReader<Pointer<Click>> approach and aligns
//! with bevy_egui's event-driven architecture.
//!
//! # Observer Pattern
//!
//! Instead of systems reading click events, we attach observers directly to entities:
//! - Pieces: `.observe(on_piece_click)` - handles piece selection
//! - Squares: `.observe(on_square_click)` - handles piece movement
//!
//! Observers are closures that run when their trigger event fires on their entity.
//! This is more performant and idiomatic than polling MessageReader in systems.
//!
//! # Bevy Egui Alignment
//!
//! This module follows bevy_egui patterns:
//! - Event-driven input handling (no polling)
//! - Observer-based entity interactions
//! - Graceful error handling for missing entities
//! - State-aware input processing
//!
//! # Error Handling
//!
//! Observer functions handle errors gracefully:
//! - Missing entities: Early return with warning
//! - Invalid game state: Input blocked when game is over
//! - Invalid selections: Logged but not panicked
//!
//! # Execution
//!
//! Observers run automatically when their trigger events fire. No system
//! registration is needed - observers are attached during entity spawning.
//!
//! # Reference
//!
//! - `reference/bevy/examples/picking/mesh_picking.rs` - Observer examples
//! - `reference/bevy/examples/picking/simple_picking.rs` - Click handling
//! - `reference/bevy_egui/src/lib.rs` - Event-driven input patterns

use crate::game::components::{Captured, HasMoved, MoveRecord, PieceMoveAnimation, SelectedPiece};
use crate::game::resources::*;
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use bevy::audio::{AudioPlayer, AudioSource};
use bevy::asset::Handle;
use bevy::picking::events::{Drag, DragDrop, DragEnd, DragStart};
use bevy::prelude::*;
use chess_engine::{do_move, is_legal_move};

fn select_piece_for_interaction(
    source: &str,
    entity: Entity,
    commands: &mut Commands,
    selection: &mut Selection,
    current_turn: &CurrentTurn,
    all_pieces: &Query<(Entity, &Piece, &HasMoved, &Transform)>,
    selected_pieces: &Query<Entity, With<SelectedPiece>>,
    game_over: &GameOverState,
    engine: &mut ChessEngine,
) -> bool {
    if game_over.is_game_over() {
        return false;
    }

    let (piece, has_moved) = match all_pieces.get(entity) {
        Ok((_, piece, has_moved, _)) => (piece, has_moved),
        Err(e) => {
            error!(
                "[INPUT] {source} failed - unable to query piece {:?}: {:?}",
                entity, e
            );
            return false;
        }
    };

    if piece.color != current_turn.color {
        return false;
    }

    // Deselect previously selected pieces for consistent visual feedback
    let deselected_count = selected_pieces.iter().count();
    if deselected_count > 0 {
        info!(
            "[INPUT] {source}: Deselecting {} previously selected piece(s)",
            deselected_count
        );
        for entity in selected_pieces.iter() {
            commands.entity(entity).remove::<SelectedPiece>();
        }
    }

    // Sync board state before generating moves
    info!(
        "[INPUT] {source}: Syncing ECS to engine before generating legal moves"
    );
    ChessEngine::sync_ecs_to_engine_with_transform(engine, all_pieces, current_turn);

    let possible_moves = engine.get_legal_moves_for_square((piece.x, piece.y), piece.color);
    info!(
        "[INPUT] {source}: Generated {} legal moves for {:?} {:?} at ({}, {})",
        possible_moves.len(),
        piece.color,
        piece.piece_type,
        piece.x,
        piece.y
    );

    selection.selected_entity = Some(entity);
    selection.selected_position = Some((piece.x, piece.y));
    selection.possible_moves = possible_moves;
    selection.drag_start = selection.selected_position;
    selection.is_dragging = false;

    commands.entity(entity).insert(SelectedPiece {
        entity,
        position: (piece.x, piece.y),
    });

    info!(
        "[INPUT] {source}: Selected {:?} {:?} at ({}, {}), has_moved={}, {} legal moves",
        piece.color,
        piece.piece_type,
        piece.x,
        piece.y,
        has_moved.moved,
        selection.possible_moves.len()
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn try_move_selected_piece_to(
    source: &str,
    target: (u8, u8),
    dropped_piece: Option<Entity>,
    commands: &mut Commands,
    selection: &mut Selection,
    pieces: &mut ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>,
        Query<(Entity, &Piece, &HasMoved)>,
        Query<(Entity, &Piece, &HasMoved, &Transform)>,
    )>,
    pending_turn: &mut PendingTurnAdvance,
    current_turn: &mut CurrentTurn,
    move_history: &mut MoveHistory,
    captured_pieces: &mut CapturedPieces,
    selected_pieces: &Query<Entity, With<SelectedPiece>>,
    fast_board: &mut FastBoardState,
    game_over: &GameOverState,
    engine: &mut ChessEngine,
    game_sounds: Option<Handle<AudioSource>>,
    capture_sound: Option<Handle<AudioSource>>,
) -> bool {
    warn!(
        "[INPUT] {source}: attempting move to ({}, {}) with selection {:?}",
        target.0, target.1, selection.selected_entity
    );

    if game_over.is_game_over() {
        return false;
    }

    if !selection.is_selected() {
        return false;
    }

    let selected_entity = selection.selected_entity.expect("checked above");
    if let Some(dropped) = dropped_piece {
        if dropped != selected_entity {
            warn!(
                "[INPUT] {source} rejected - dropped entity {:?} does not match selected {:?}",
                dropped, selected_entity
            );
            return false;
        }
    }

    let move_plan = {
        let pieces_read = pieces.p1();

        // Keep engine in sync for legality checks
        info!(
            "[INPUT] {source}: Syncing ECS to engine before move validation"
        );
        engine.sync_ecs_to_engine(&pieces_read, current_turn);

        let (_, piece, has_moved) = match pieces_read.get(selected_entity) {
            Ok(data) => data,
            Err(e) => {
                error!(
                    "[INPUT] {source} failed - unable to query selected piece {:?}: {:?}",
                    selected_entity, e
                );
                return false;
            }
        };

        // Ensure target was presented as a legal move to the player
        if !selection.possible_moves.contains(&target) {
            warn!(
                "[INPUT] {source}: Move rejected - square ({}, {}) not in legal moves list ({} moves available)",
                target.0, target.1, selection.possible_moves.len()
            );
            if !selection.possible_moves.is_empty() {
                debug!(
                    "[INPUT] {source}: Available moves: {:?}",
                    selection.possible_moves
                );
            }
            return false;
        }

        let src_index = ChessEngine::square_to_index(piece.x, piece.y);
        let dst_index = ChessEngine::square_to_index(target.0, target.1);
        let engine_color = ChessEngine::piece_color_to_engine(piece.color);

        info!(
            "[INPUT] {source}: Validating move ({}, {}) -> ({}, {}) with engine",
            piece.x, piece.y, target.0, target.1
        );
        if !is_legal_move(&mut engine.game, src_index, dst_index, engine_color) {
            warn!(
                "[INPUT] {source}: Move rejected - engine says move ({}, {}) -> ({}, {}) is illegal",
                piece.x, piece.y, target.0, target.1
            );
            return false;
        }
        info!(
            "[INPUT] {source}: Move validated by engine - proceeding with execution"
        );

        let mut captured_piece = None;
        let mut captured_piece_color = None;
        let mut captured_entity = None;
        for (entity, other_piece, _) in pieces_read.iter() {
            if entity != selected_entity && other_piece.x == target.0 && other_piece.y == target.1 {
                captured_piece = Some(other_piece.piece_type);
                captured_piece_color = Some(other_piece.color);
                captured_entity = Some(entity);
                break;
            }
        }

        (
            src_index,
            dst_index,
            (piece.x, piece.y),
            piece.piece_type,
            piece.color,
            !has_moved.moved,
            captured_entity,
            captured_piece,
            captured_piece_color,
        )
    };

    let (
        src_index,
        dst_index,
        from_pos,
        piece_type,
        piece_color,
        was_first_move,
        captured_entity,
        captured_piece,
        captured_piece_color,
    ) = move_plan;

    // Execute move in engine
    do_move(&mut engine.game, src_index, dst_index, true);

    // Move captured piece to capture zone instead of despawning
    if let Some(entity) = captured_entity {
        if let (Some(piece_type), Some(piece_color)) = (captured_piece, captured_piece_color) {
            info!(
                "[INPUT] {source}: capture of {:?} {:?} at ({}, {})",
                piece_color, piece_type, target.0, target.1
            );

            // Play capture sound (only if handle is provided)
            if let Some(sound_handle) = capture_sound {
                // Audio will be played by Bevy's audio system if the asset is loaded
                // If the asset failed to load, Bevy's error handler will catch it
                commands.spawn(AudioPlayer::new(sound_handle));
            }

            let count_of_same_type = match piece_color {
                crate::rendering::pieces::PieceColor::White => captured_pieces
                    .black_captured
                    .iter()
                    .filter(|&&p| p == piece_type)
                    .count(),
                crate::rendering::pieces::PieceColor::Black => captured_pieces
                    .white_captured
                    .iter()
                    .filter(|&&p| p == piece_type)
                    .count(),
            };

            captured_pieces.add_capture(piece_color, piece_type);

            let capture_pos = crate::rendering::pieces::calculate_capture_position(
                piece_color,
                piece_type,
                count_of_same_type,
            );

            commands
                .entity(entity)
                .insert((Transform::from_translation(capture_pos), Captured));

        }
    } else {
        // Play move sound (only if no capture)
        if let Some(sound_handle) = game_sounds {
            // Audio will be played by Bevy's audio system if the asset is loaded
            // If the asset failed to load, Bevy's error handler will catch it
            commands.spawn(AudioPlayer::new(sound_handle));
        }
    }

    // Update ECS piece state
    if let Ok((_, mut piece, mut has_moved)) = pieces.p0().get_mut(selected_entity) {
        let move_record = MoveRecord {
            piece_type,
            piece_color,
            from: from_pos,
            to: target,
            captured: captured_piece,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };
        move_history.add_move(move_record);

        piece.x = target.0;
        piece.y = target.1;

        fast_board.mark_dirty();

        commands
            .entity(selected_entity)
            .insert(PieceMoveAnimation::new(
                Vec3::new(from_pos.0 as f32, 0.0, from_pos.1 as f32),
                Vec3::new(target.0 as f32, 0.0, target.1 as f32),
                0.25,
            ));

        has_moved.moved = true;
        has_moved.move_count += 1;

        info!(
            "[INPUT] {source}: {:?} {:?} moved ({}, {}) -> ({}, {}){}",
            piece_color,
            piece_type,
            from_pos.0,
            from_pos.1,
            target.0,
            target.1,
            if was_first_move { " [FIRST MOVE]" } else { "" }
        );

        if captured_piece.is_some() {
            info!("[INPUT] {source}: move type capture");
        }

        info!(
            "[INPUT] {source}: move count now {} for this piece",
            has_moved.move_count
        );
    } else {
        error!(
            "[INPUT] {source} - unable to mutate selected piece {:?} after move",
            selected_entity
        );
        return false;
    }

    // Sync engine back to ECS for derived state (castling rights, etc.)
    info!(
        "[INPUT] {source}: Syncing engine to ECS after move execution"
    );
    {
        let mut pieces_query_mut = pieces.p0();
        engine.sync_engine_to_ecs(commands, &mut pieces_query_mut);
    }

    if pending_turn.request(piece_color) {
        info!(
            "[INPUT] {source}: Queued turn advance for {:?} after animation",
            piece_color
        );
    }

    info!(
        "[INPUT] {source}: Clearing selection after successful move"
    );
    for entity in selected_pieces.iter() {
        commands.entity(entity).remove::<SelectedPiece>();
    }

    selection.clear();

    true
}

/// Observer function called when a piece is clicked
///
/// Handles piece selection for the chess game. This observer is attached to
/// piece entities during spawning and runs automatically when a piece is clicked.
///
/// # Behavior
///
/// 1. Verifies it's the current player's piece (blocks selecting opponent pieces)
/// 2. Syncs ECS board state to engine for move generation
/// 3. Calculates legal moves using the chess engine
/// 4. Updates Selection resource with selected piece and valid moves
/// 5. Adds SelectedPiece component for visual feedback
///
/// # Error Handling
///
/// - Returns early if game is over (no input allowed)
/// - Returns early if piece doesn't belong to current player
/// - Logs warnings for invalid selections but doesn't panic
///
/// # Examples
///
/// ```rust,ignore
/// // Attach observer during piece spawning
/// commands.spawn(PieceBundle { /* ... */ })
///     .observe(on_piece_click);
/// ```
pub fn on_piece_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    mut selection: ResMut<Selection>,
    current_turn: Res<CurrentTurn>,
    all_pieces: Query<(Entity, &Piece, &HasMoved, &Transform)>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
    game_over: Res<GameOverState>,
    mut engine: ResMut<ChessEngine>,
) {
    // Only handle left-click (Primary button) for piece selection
    // Right-click (Secondary) is used for piece viewer
    use bevy::picking::pointer::PointerButton;
    if click.event.button != PointerButton::Primary {
        return;
    }

    // Check if clicking the same piece that's already selected (deselect)
    if selection.selected_entity == Some(click.entity) {
        info!(
            "[INPUT] Clicked same piece - deselecting"
        );
        // Deselect by clearing selection
        for entity in selected_pieces.iter() {
            commands.entity(entity).remove::<SelectedPiece>();
        }
        selection.clear();
        return;
    }

    if select_piece_for_interaction(
        "CLICK",
        click.entity,
        &mut commands,
        &mut selection,
        &*current_turn,
        &all_pieces,
        &selected_pieces,
        &*game_over,
        engine.as_mut(),
    ) {
        // ensure drag origin is set for future drags
        selection.drag_start = selection.selected_position;
        info!(
            "[INPUT] Piece selected via click"
        );
    }
}

/// Observer function triggered when a dragged piece is dropped on a square
pub fn on_square_drag_drop(
    drop: On<Pointer<DragDrop>>,
    mut commands: Commands,
    square_query: Query<&Square>,
    mut selection: ResMut<Selection>,
    mut pieces: ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>,
        Query<(Entity, &Piece, &HasMoved)>,
        Query<(Entity, &Piece, &HasMoved, &Transform)>,
    )>,
    mut current_turn: ResMut<CurrentTurn>,
    mut move_history: ResMut<MoveHistory>,
    mut captured_pieces: ResMut<CapturedPieces>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
    mut fast_board: ResMut<FastBoardState>,
    game_over: Res<GameOverState>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut engine: ResMut<ChessEngine>,
    game_sounds: Option<Res<GameSounds>>,
) {
    use bevy::picking::pointer::PointerButton;
    if drop.event.button != PointerButton::Primary {
        return;
    }

    warn!(
        "[INPUT] Drop target entity: {:?}, dropped piece: {:?}",
        drop.entity, drop.event.dropped
    );

    match square_query.get(drop.entity) {
        Ok(square) => {
            let target = (square.x, square.y);
            warn!(
                "[INPUT] Drag drop target position: ({}, {})",
                target.0, target.1
            );

            let _ = try_move_selected_piece_to(
                "SQUARE_DROP",
                target,
                Some(drop.event.dropped),
                &mut commands,
                &mut selection,
                &mut pieces,
                pending_turn.as_mut(),
                current_turn.as_mut(),
                move_history.as_mut(),
                captured_pieces.as_mut(),
                &selected_pieces,
                fast_board.as_mut(),
                &*game_over,
                engine.as_mut(),
                game_sounds.as_ref().map(|s| s.move_piece.clone()),
                game_sounds.as_ref().map(|s| s.capture_piece.clone()),
            );
        }
        Err(e) => {
            error!(
                "[INPUT] ERROR: Failed to query square entity {:?}: {:?}",
                drop.entity, e
            );
        }
    }
}

/// Observer function called when a piece drag begins
pub fn on_piece_drag_start(
    drag: On<Pointer<DragStart>>,
    mut commands: Commands,
    mut selection: ResMut<Selection>,
    current_turn: Res<CurrentTurn>,
    all_pieces: Query<(Entity, &Piece, &HasMoved, &Transform)>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
    game_over: Res<GameOverState>,
    mut engine: ResMut<ChessEngine>,
) {
    use bevy::picking::pointer::PointerButton;
    if drag.event.button != PointerButton::Primary {
        return;
    }


    if selection.selected_entity == Some(drag.entity) {
        selection.begin_drag();
        selection.drag_start = selection.selected_position;
        warn!(
            "[INPUT] Drag start using existing selection {:?}",
            drag.entity
        );
        return;
    }

    if select_piece_for_interaction(
        "DRAG_START",
        drag.entity,
        &mut commands,
        &mut selection,
        &*current_turn,
        &all_pieces,
        &selected_pieces,
        &*game_over,
        engine.as_mut(),
    ) {
        selection.begin_drag();
        warn!("[INPUT] Drag start selection initialized");
    }
}

/// Observer function called during a piece drag to update its position
pub fn on_piece_drag(
    drag: On<Pointer<Drag>>,
    mut transforms: Query<&mut Transform>,
    selection: ResMut<Selection>,
    cameras: Query<(&Camera, &GlobalTransform)>,
) {
    use bevy::picking::pointer::PointerButton;
    if drag.event.button != PointerButton::Primary {
        return;
    }

    if !selection.is_dragging || selection.selected_entity != Some(drag.entity) {
        return;
    }

    // Update piece position to follow the cursor
    // Convert screen position to world coordinates using the camera
    if let Ok(mut transform) = transforms.get_mut(drag.entity) {
        // Try to get world position from pointer location
        let viewport_pos = drag.pointer_location.position;
        
        // Find the active camera and convert viewport position to world position
        for (camera, camera_transform) in cameras.iter() {
            if let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_pos) {
                // Intersect ray with board plane at y = 0.0 (board level)
                // Use InfinitePlane3d for proper plane intersection
                let board_plane = InfinitePlane3d::new(Vec3::Y);
                let board_origin = Vec3::ZERO;
                
                if let Some(distance) = ray.intersect_plane(board_origin, board_plane) {
                    let world_pos = ray.get_point(distance);
                    // Lift piece slightly above board during drag for visual feedback
                    let lift_height = 0.5;
                    transform.translation = Vec3::new(world_pos.x, lift_height, world_pos.z);
                    break;
                }
            }
        }
    }
}

/// Observer function called when a piece drag ends (without a valid drop)
pub fn on_piece_drag_end(
    drag: On<Pointer<DragEnd>>,
    mut selection: ResMut<Selection>,
    mut transforms: Query<&mut Transform>,
    pieces: Query<&Piece>,
) {
    use bevy::picking::pointer::PointerButton;
    if drag.event.button != PointerButton::Primary {
        return;
    }

    if selection.is_dragging {
        warn!(
            "[INPUT] ========== PIECE DRAG END ==========\n[INPUT] Drag ended for entity {:?}",
            drag.entity
        );
        selection.end_drag();
        
        // Reset piece to its board position if no valid move was made
        if let Some(selected_entity) = selection.selected_entity {
            if selected_entity == drag.entity {
                if let Ok(piece) = pieces.get(selected_entity) {
                    if let Ok(mut transform) = transforms.get_mut(selected_entity) {
                        transform.translation = Vec3::new(piece.x as f32, 0.0, piece.y as f32);
                    }
                }
                selection.drag_start = selection.selected_position;
            }
        }
    }
}

/// Observer function called when a square is clicked
///
/// Handles piece movement when a square is clicked. This observer is attached to
/// square entities during spawning and runs automatically when a square is clicked.
///
/// # Behavior
///
/// 1. Checks if clicked square contains a piece (prioritizes piece selection)
/// 2. If square has current player's piece, selects it instead of moving
/// 3. If square is empty and piece is selected, executes move
/// 4. If square is empty and no piece selected, clears selection
/// 5. Verifies the clicked square is a valid move destination
/// 6. Executes the move (updates piece position, handles captures)
/// 7. Records the move in move history with full metadata
/// 8. Updates game timer and switches turns
/// 9. Syncs board state between ECS and engine
///
/// # Error Handling
///
/// - Returns early if game is over (no input allowed)
/// - Returns early if no piece is selected and square is empty
/// - Returns early if move is invalid (not in possible_moves)
/// - Logs warnings for invalid moves but doesn't panic
///
/// # Examples
///
/// ```rust,ignore
/// // Attach observer during square spawning
/// commands.spawn(SquareBundle { /* ... */ })
///     .observe(on_square_click);
/// ```
pub fn on_square_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    square_query: Query<&Square>,
    mut selection: ResMut<Selection>,
    mut pieces: ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>,
        Query<(Entity, &Piece, &HasMoved)>,
        Query<(Entity, &Piece, &HasMoved, &Transform)>,
    )>,
    mut current_turn: ResMut<CurrentTurn>,
    mut move_history: ResMut<MoveHistory>,
    mut captured_pieces: ResMut<CapturedPieces>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
    mut fast_board: ResMut<FastBoardState>,
    game_over: Res<GameOverState>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut engine: ResMut<ChessEngine>,
    game_sounds: Option<Res<GameSounds>>,
) {
    use bevy::picking::pointer::PointerButton;
    if click.event.button != PointerButton::Primary {
        return;
    }

    if game_over.is_game_over() {
        return;
    }

    let square = match square_query.get(click.entity) {
        Ok(s) => s,
        Err(e) => {
            error!(
                "[INPUT] ERROR: Failed to query square entity {:?}: {:?}",
                click.entity, e
            );
            return;
        }
    };

    let target = (square.x, square.y);
    info!(
        "[INPUT] Square click at position: ({}, {})",
        target.0, target.1
    );

    // PRIORITY 1: Check if square contains a piece
    // If it does and it's the current player's piece, select it instead of moving
    let pieces_with_transform = pieces.p2();
    for (piece_entity, piece, _, _) in pieces_with_transform.iter() {
        if piece.x == target.0 && piece.y == target.1 {
            // Square contains a piece
            if piece.color == current_turn.color {
                // It's the current player's piece - select it instead of moving
                info!(
                    "[INPUT] Square click on own piece at ({}, {}) - selecting piece instead of moving",
                    target.0, target.1
                );
                
                // Check if this is the same piece that's already selected (deselect)
                if selection.selected_entity == Some(piece_entity) {
                    info!(
                        "[INPUT] Clicked same piece - deselecting"
                    );
                    // Deselect by clearing selection
                    for entity in selected_pieces.iter() {
                        commands.entity(entity).remove::<SelectedPiece>();
                    }
                    selection.clear();
                    return;
                }
                
                // Select this piece
                if select_piece_for_interaction(
                    "SQUARE_CLICK_PIECE",
                    piece_entity,
                    &mut commands,
                    &mut selection,
                    &*current_turn,
                    &pieces_with_transform,
                    &selected_pieces,
                    &*game_over,
                    engine.as_mut(),
                ) {
                    info!(
                        "[INPUT] Selected piece at ({}, {}) from square click",
                        target.0, target.1
                    );
                }
                return; // Don't process as move - we selected a piece
            } else {
                // It's the opponent's piece - ignore (can't select opponent pieces)
                info!(
                    "[INPUT] Square click on opponent piece at ({}, {}) - ignoring",
                    target.0, target.1
                );
                return;
            }
        }
    }

    // PRIORITY 2: Square is empty
    // If no piece is selected, clear selection and return
    if !selection.is_selected() {
        info!(
            "[INPUT] Square click on empty square with no selection - clearing any stale selection"
        );
        // Clear any stale selection state
        for entity in selected_pieces.iter() {
            commands.entity(entity).remove::<SelectedPiece>();
        }
        selection.clear();
        return;
    }

    // PRIORITY 3: Square is empty and piece is selected - execute move
    info!(
        "[INPUT] Square click on empty square ({}, {}) with piece selected - attempting move",
        target.0, target.1
    );

    let _ = try_move_selected_piece_to(
        "SQUARE_CLICK",
        target,
        None,
        &mut commands,
        &mut selection,
        &mut pieces,
        pending_turn.as_mut(),
        current_turn.as_mut(),
        move_history.as_mut(),
        captured_pieces.as_mut(),
        &selected_pieces,
        fast_board.as_mut(),
        &*game_over,
        engine.as_mut(),
        game_sounds.as_ref().map(|s| s.move_piece.clone()),
        game_sounds.as_ref().map(|s| s.capture_piece.clone()),
    );
}
