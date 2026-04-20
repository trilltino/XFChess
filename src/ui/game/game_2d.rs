//! 2D game board rendering using egui
//!
//! Provides a lightweight 2D chess board interface using egui's immediate mode GUI.
//! This allows players to play chess with a traditional 2D view while maintaining
//! full compatibility with the existing game state and networking systems.

use crate::rendering::pieces::Piece;
use bevy::prelude::*;
use bevy_egui::egui;


/// Square colors for the chess board
fn square_color(file: u8, rank: u8) -> egui::Color32 {
    // Standard chess board coloring: a1 is dark
    if (file + rank) % 2 == 0 {
        egui::Color32::from_rgb(240, 217, 181) // Light squares
    } else {
        egui::Color32::from_rgb(181, 136, 99) // Dark squares
    }
}

/// Highlight colors for move indicators
fn highlight_color(highlight_type: HighlightType) -> egui::Color32 {
    match highlight_type {
        HighlightType::Selected => egui::Color32::from_rgba_unmultiplied(255, 255, 0, 100),
        HighlightType::LegalMove => egui::Color32::from_rgba_unmultiplied(0, 255, 0, 80),
        HighlightType::Check => egui::Color32::from_rgba_unmultiplied(255, 0, 0, 120),
        HighlightType::LastMove => egui::Color32::from_rgba_unmultiplied(255, 255, 255, 60),
    }
}

/// Types of square highlights
#[derive(Debug, Clone, Copy)]
enum HighlightType {
    Selected,
    LegalMove,
    Check,
    LastMove,
}

/// Main 2D board rendering system
pub fn render_2d_board(
    mut input_params: crate::game::systems::input::InputSystemParams,
    mut contexts: bevy_egui::EguiContexts,
    sprite_handles: Option<Res<crate::rendering::pieces::PieceSpriteHandles>>,
    game_phase: Res<crate::game::resources::CurrentGamePhase>,
) {
    // Pre-register all piece textures to avoid double-borrowing contexts in the CentralPanel closure
    let mut texture_ids = std::collections::HashMap::new();
    if let Some(ref handles) = sprite_handles {
        for &color in &[crate::rendering::pieces::PieceColor::White, crate::rendering::pieces::PieceColor::Black] {
            for &ptype in &[
                crate::game::components::PieceType::Pawn,
                crate::game::components::PieceType::Knight,
                crate::game::components::PieceType::Bishop,
                crate::game::components::PieceType::Rook,
                crate::game::components::PieceType::Queen,
                crate::game::components::PieceType::King,
            ] {
                let handle = handles.get(ptype, color);
                let id = contexts.add_image(bevy_egui::EguiTextureHandle::Strong(handle));
                texture_ids.insert((ptype, color), id);
            }
        }
    }

    let ctx = contexts.ctx_mut().expect("Failed to get egui context");

    // Don't render if game is over (let 3D handle endgame screen)
    if input_params.game_over.is_game_over() {
        return;
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            // Calculate board size based on available space
            let available_size = ui.available_size();
            let board_size = available_size.x.min(available_size.y * 0.9);
            let square_size = board_size / 8.0;

            // Create the chess board interaction area
            let response = ui.allocate_response(
                egui::Vec2::new(board_size, board_size),
                egui::Sense::click_and_drag(),
            );

            // Draw the board and pieces
            let painter = ui.painter();
            let board_rect = response.rect;

            // Handle drag-and-drop start
            if response.drag_started() {
                let mouse_pos = response.interact_pointer_pos().unwrap_or(board_rect.center());
                let local_pos = mouse_pos - board_rect.min;
                let raw_file = (local_pos.x / square_size) as u8;
                let rank = 7 - (local_pos.y / square_size) as u8;
                let file = 7 - raw_file;
                
                if file < 8 && rank < 8 {
                    let pieces = input_params.pieces.p1();
                    if let Some((entity, piece)) = crate::game::systems::shared::find_piece_on_square(&pieces, (file, rank)) {
                        // Only start drag if it's our piece and our turn
                        if piece.color == input_params.current_turn.color && crate::game::systems::input::can_move_color(&input_params, piece.color) {
                            input_params.selection.selected_entity = Some(entity);
                            input_params.selection.selected_position = Some((file, rank));
                            input_params.selection.begin_drag();
                            // Calculate legal moves for dragging
                            let legal_moves = input_params.engine.get_legal_moves_for_square((file, rank), piece.color);
                            input_params.selection.possible_moves = legal_moves;
                        }
                    }
                }
            }

            // Draw squares
            for rank in 0..8 {
                for file in 0..8 {
                    let square_rect = egui::Rect::from_min_size(
                        board_rect.min + egui::Vec2::new((7 - file) as f32 * square_size, (7 - rank) as f32 * square_size),
                        egui::Vec2::new(square_size, square_size),
                    );

                    // Draw square background
                    painter.rect_filled(square_rect, 0.0, square_color(file, rank));

                    // Highlight Last Move
                    if let Some(last_move) = input_params.move_history.last_move() {
                        if (last_move.from.0 == file && last_move.from.1 == rank) || 
                           (last_move.to.0 == file && last_move.to.1 == rank) {
                            painter.rect_filled(square_rect, 0.0, highlight_color(HighlightType::LastMove));
                        }
                    }

                    // Highlight King in Check
                    if game_phase.0 == crate::game::components::GamePhase::Check {
                        let pieces = input_params.pieces.p1();
                        for (_, piece, _, _) in pieces.iter() {
                            if piece.piece_type == crate::game::components::PieceType::King && 
                               piece.color == input_params.current_turn.color &&
                               piece.x == file && piece.y == rank {
                                painter.rect_filled(square_rect, 0.0, highlight_color(HighlightType::Check));
                            }
                        }
                    }

                    // Check for highlights
                    if let Some(selected) = input_params.selection.selected_position {
                        if selected == (file, rank) {
                            painter.rect_filled(square_rect, 0.0, highlight_color(HighlightType::Selected));
                        }
                    }

                    // Highlight legal moves
                    for &(legal_file, legal_rank) in &input_params.selection.possible_moves {
                        if legal_file == file && legal_rank == rank {
                            painter.rect_filled(square_rect, 0.0, highlight_color(HighlightType::LegalMove));
                        }
                    }
                }
            }

            // Draw pieces
            let pieces = input_params.pieces.p1();
            let mut drag_piece: Option<(Piece, egui::Rect)> = None;

            for (_, piece, _, _) in pieces.iter() {
                let is_being_dragged = input_params.selection.is_dragging && 
                                      input_params.selection.selected_position == Some((piece.x, piece.y));

                // Debug: Log first few pieces to verify positions
                if piece.piece_type == crate::game::components::PieceType::Pawn && piece.color == crate::rendering::pieces::PieceColor::White {
                    debug!("[2D_RENDER] White pawn at ({}, {}) - 2D pos: ({}, {})", 
                        piece.x, piece.y,
                        piece.x as f32 * square_size,
                        (7 - piece.y) as f32 * square_size);
                }

                let piece_rect = if is_being_dragged {
                    let mouse_pos = ui.ctx().pointer_latest_pos().unwrap_or(board_rect.center());
                    let rect = egui::Rect::from_center_size(mouse_pos, egui::Vec2::new(square_size, square_size));
                    drag_piece = Some((*piece, rect));
                    continue; // Draw dragging piece last (on top)
                } else {
                    egui::Rect::from_min_size(
                        board_rect.min + egui::Vec2::new((7 - piece.x) as f32 * square_size, (7 - piece.y) as f32 * square_size),
                        egui::Vec2::new(square_size, square_size),
                    )
                };

                if let Some(texture_id) = texture_ids.get(&(piece.piece_type, piece.color)) {
                    painter.image(
                        *texture_id,
                        piece_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }

            // Draw dragging piece on top
            if let Some((piece, rect)) = drag_piece {
                if let Some(texture_id) = texture_ids.get(&(piece.piece_type, piece.color)) {
                    painter.image(
                        *texture_id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200), // Slightly transparent
                    );
                }
            }

            // Handle input
            if response.clicked() {
                let mouse_pos = response.interact_pointer_pos().unwrap_or(board_rect.center());
                let local_pos = mouse_pos - board_rect.min;
                
                let raw_file = (local_pos.x / square_size) as u8;
                let rank = 7 - (local_pos.y / square_size) as u8;
                let file = 7 - raw_file;

                if file < 8 && rank < 8 {
                    handle_2d_click(file as u8, rank as u8, &mut input_params);
                }
            } else if response.drag_stopped() {
                let mouse_pos = ui.ctx().pointer_latest_pos().unwrap_or(board_rect.center());
                let local_pos = mouse_pos - board_rect.min;
                let raw_file = (local_pos.x / square_size) as u8;
                let rank = 7 - (local_pos.y / square_size) as u8;
                let file = 7 - raw_file;

                debug!("[2D_DRAG] Mouse: {:?}, Local: {:?}, Calculated: file={}, rank={}", 
                    mouse_pos, local_pos, file, rank);

                if file < 8 && rank < 8 {
                    handle_2d_click(file as u8, rank as u8, &mut input_params);
                }
                input_params.selection.end_drag();
            }
        });
    });
}

/// Handle mouse clicks on the 2D board
fn handle_2d_click(
    file: u8,
    rank: u8,
    params: &mut crate::game::systems::input::InputSystemParams,
) {
    use crate::game::systems::input::{can_move_color, try_select_piece, try_move_sequence};
    use crate::game::systems::shared::{find_piece_on_square, CapturedTarget};

    let target_pos = (file, rank);
    debug!("[2D] Clicked square at ({}, {})", file, rank);

    // Find if there's a piece on this square
    let occupant = {
        let q = params.pieces.p1();
        find_piece_on_square(&q, target_pos)
    };

    if let Some((piece_entity, piece)) = occupant {
        // Case 1: Clicked our own piece -> Select
        if piece.color == params.current_turn.color {
            if can_move_color(params, piece.color) {
                try_select_piece(params, piece_entity, piece, true);
                return;
            }
        }

        // Case 2: Clicked enemy piece -> Capture
        if params.selection.is_selected() && piece.color != params.current_turn.color {
            try_move_sequence(params, target_pos, Some(CapturedTarget {
                entity: piece_entity,
                piece_type: piece.piece_type,
                color: piece.color,
            }), "2d_piece_capture");
            return;
        }
    }

    // Case 3: Clicked empty square -> Move
    if params.selection.is_selected() {
        try_move_sequence(params, target_pos, None, "2d_square_click");
    }
}
