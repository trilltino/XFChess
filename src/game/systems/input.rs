//! Observer-based input handling for piece selection and movement
//!
//! Implements Bevy 0.17's idiomatic `.observe()` pattern for pointer interactions.
//! This replaces the deprecated MessageReader<Pointer<Click>> approach.
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
//! # Reference
//!
//! - `reference/bevy/examples/picking/mesh_picking.rs` - Observer examples
//! - `reference/bevy/examples/picking/simple_picking.rs` - Click handling

use bevy::prelude::*;
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use crate::game::resources::*;
use crate::game::rules::{BoardState, get_possible_moves};
use crate::game::components::{HasMoved, SelectedPiece, MoveRecord};

/// Observer function called when a piece is clicked
///
/// Handles:
/// 1. Verifying it's the current player's piece
/// 2. Calculating legal moves
/// 3. Updating Selection resource
/// 4. Adding SelectedPiece component for visual feedback
pub fn on_piece_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    mut selection: ResMut<Selection>,
    current_turn: Res<CurrentTurn>,
    piece_query: Query<(&Piece, &HasMoved)>,
    all_pieces: Query<(Entity, &Piece, &HasMoved, &Transform)>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
) {
    let clicked_entity = click.entity;

    // Get the piece that was clicked
    if let Ok((piece, has_moved)) = piece_query.get(clicked_entity) {
        // Only allow selecting pieces of the current player's color
        if piece.color == current_turn.color {
            // Remove SelectedPiece from previously selected pieces
            let deselected_count = selected_pieces.iter().count();
            if deselected_count > 0 {
                info!("[INPUT] Deselecting {} previously selected piece(s)", deselected_count);
            }
            for entity in selected_pieces.iter() {
                commands.entity(entity).remove::<SelectedPiece>();
            }

            // Build board state for move calculation
            let board_state = BoardState {
                pieces: all_pieces
                    .iter()
                    .map(|(entity, piece, _, _)| (entity, *piece, (piece.x, piece.y)))
                    .collect(),
            };

            // Calculate possible moves
            let possible_moves = get_possible_moves(
                piece.piece_type,
                piece.color,
                (piece.x, piece.y),
                &board_state,
                has_moved.moved,
            );

            // Log information before moving possible_moves
            info!("[INPUT] ========== PIECE SELECTED ==========");
            info!("[INPUT] Player: {:?} | Piece: {:?} | Position: ({}, {}) | Has Moved: {}",
                piece.color, piece.piece_type, piece.x, piece.y, has_moved.moved);
            info!("[INPUT] Legal Moves: {} available -> {:?}", possible_moves.len(), possible_moves);
            info!("[INPUT] Board State: {} total pieces on board", board_state.pieces.len());

            // Update selection resource (move Vec instead of cloning)
            selection.selected_entity = Some(clicked_entity);
            selection.selected_position = Some((piece.x, piece.y));
            selection.possible_moves = possible_moves;

            // Add SelectedPiece component for visual feedback
            commands.entity(clicked_entity).insert(SelectedPiece {
                entity: clicked_entity,
                position: (piece.x, piece.y),
            });
        } else {
            warn!("[INPUT] INVALID SELECTION: {:?} {:?} at ({}, {}) - Not {:?}'s turn!",
                piece.color, piece.piece_type, piece.x, piece.y, current_turn.color);
        }
    }
}

/// Observer function called when a square is clicked
///
/// Handles:
/// 1. Checking if a piece is selected
/// 2. Verifying the clicked square is a valid move
/// 3. Executing the move (capturing, updating position)
/// 4. Recording the move in history
/// 5. Switching turns
pub fn on_square_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    square_query: Query<&Square>,
    selection: Res<Selection>,
    mut pieces_query: Query<(Entity, &mut Piece, &mut HasMoved)>,
    mut current_turn: ResMut<CurrentTurn>,
    mut move_history: ResMut<MoveHistory>,
    mut game_timer: ResMut<GameTimer>,
    selected_pieces: Query<Entity, With<SelectedPiece>>,
) {
    if !selection.is_selected() {
        return;
    }

    let clicked_square_entity = click.entity;

    if let Ok(square) = square_query.get(clicked_square_entity) {
        let target = (square.x, square.y);

        // Check if this square is a valid move destination
        if selection.possible_moves.contains(&target) {
            if let Some(selected_entity) = selection.selected_entity {
                // Check for captures (before any mutable borrows)
                let mut captured_piece = None;
                let mut captured_piece_color = None;
                let mut captured_entity = None;
                for (entity, piece, _) in pieces_query.iter() {
                    if piece.x == target.0 && piece.y == target.1 {
                        captured_piece = Some(piece.piece_type);
                        captured_piece_color = Some(piece.color);
                        captured_entity = Some(entity);
                        break;
                    }
                }

                // Despawn captured piece
                if let Some(entity) = captured_entity {
                    if let (Some(piece_type), Some(piece_color)) = (captured_piece, captured_piece_color) {
                        info!("[INPUT] CAPTURE! {:?} {:?} taken at ({}, {})",
                            piece_color, piece_type, target.0, target.1);
                    }
                    commands.entity(entity).despawn();
                }

                // Execute the move
                if let Ok((_, mut piece, mut has_moved)) = pieces_query.get_mut(selected_entity) {
                    let from_pos = (piece.x, piece.y);

                    // Record the move
                    let move_record = MoveRecord {
                        piece_type: piece.piece_type,
                        piece_color: piece.color,
                        from: (piece.x, piece.y),
                        to: target,
                        captured: captured_piece,
                        is_castling: false,
                        is_en_passant: false,
                        is_check: false,
                        is_checkmate: false,
                    };
                    move_history.add_move(move_record);

                    // Update piece position
                    piece.x = target.0;
                    piece.y = target.1;

                    // Mark piece as moved
                    let was_first_move = !has_moved.moved;
                    has_moved.moved = true;
                    has_moved.move_count += 1;

                    info!("[INPUT] ========== MOVE EXECUTED ==========");
                    info!("[INPUT] {:?} {:?}: ({}, {}) -> ({}, {}){}",
                        piece.color, piece.piece_type,
                        from_pos.0, from_pos.1, target.0, target.1,
                        if was_first_move { " [FIRST MOVE]" } else { "" });
                    if captured_piece.is_some() {
                        info!("[INPUT] Move Type: CAPTURE");
                    }
                    info!("[INPUT] Move Count: {} for this piece", has_moved.move_count);

                    // Apply Fischer increment
                    let time_before = match piece.color {
                        crate::rendering::pieces::PieceColor::White => game_timer.white_time_left,
                        crate::rendering::pieces::PieceColor::Black => game_timer.black_time_left,
                    };
                    game_timer.apply_increment(piece.color);
                    let time_after = match piece.color {
                        crate::rendering::pieces::PieceColor::White => game_timer.white_time_left,
                        crate::rendering::pieces::PieceColor::Black => game_timer.black_time_left,
                    };
                    info!("[INPUT] Timer: +{:.1}s increment ({:.1}s -> {:.1}s)",
                        time_after - time_before, time_before, time_after);

                    // Switch turns
                    current_turn.switch();
                    info!("[INPUT] Turn Switch: Now {:?}'s turn | Move #{}",
                        current_turn.color, current_turn.move_number);
                    info!("[INPUT] Time Remaining: White {:.1}s | Black {:.1}s",
                        game_timer.white_time_left, game_timer.black_time_left);

                    // Remove SelectedPiece component
                    for entity in selected_pieces.iter() {
                        commands.entity(entity).remove::<SelectedPiece>();
                    }
                }
            }
        } else {
            warn!("[INPUT] INVALID MOVE ATTEMPT: Square ({}, {}) not in legal moves {:?}",
                target.0, target.1, selection.possible_moves);
        }
    }
}
