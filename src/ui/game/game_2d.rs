//! 2D game board rendering using egui
//!
//! Provides a lightweight 2D chess board interface using egui's immediate mode GUI.
//! This allows players to play chess with a traditional 2D view while maintaining
//! full compatibility with the existing game state and networking systems.

use crate::core::states::GameMode;
use crate::game::resources::{CurrentTurn, Players};
use crate::game::components::FadingCapture;
use crate::game::systems::camera::get_is_black_view;
use crate::game::systems::input::{
    can_move_color, clear_selection_state, is_human_turn, try_move_sequence, try_select_piece,
    InputSystemParams,
};
use crate::game::systems::shared::CapturedTarget;
use crate::game::view_mode::ViewMode;
use crate::rendering::pieces::{PieceColor, PieceType};
use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::HashMap;

#[allow(dead_code)]
fn ai_level_label(difficulty: crate::game::ai::resource::AIDifficulty) -> &'static str {
    match difficulty {
        crate::game::ai::resource::AIDifficulty::Level1 => "400 ELO",
        crate::game::ai::resource::AIDifficulty::Level2 => "700 ELO",
        crate::game::ai::resource::AIDifficulty::Level3 => "1000 ELO",
        crate::game::ai::resource::AIDifficulty::Level4 => "1300 ELO",
        crate::game::ai::resource::AIDifficulty::Level5 => "1600 ELO",
        crate::game::ai::resource::AIDifficulty::Level6 => "1900 ELO",
        crate::game::ai::resource::AIDifficulty::Level7 => "2200 ELO",
        crate::game::ai::resource::AIDifficulty::Level8 => "2500+ ELO",
    }
}

#[allow(dead_code)]
fn player_card_text(name: &str, elo: &str) -> String {
    if elo.is_empty() {
        name.to_string()
    } else {
        format!("{} - {}", name, elo)
    }
}

    // The local is_black_view function is removed in favor of the shared helper in camera.rs


/// Convert board (file, rank) to screen offset within the board widget.
/// White view: a-file on left, rank 1 at bottom.
/// Black view: h-file on left, rank 8 at bottom.
fn board_to_screen(file: u8, rank: u8, black_view: bool, square_size: f32) -> egui::Vec2 {
    let (sx, sy) = if black_view {
        (7 - file, rank)
    } else {
        (file, 7 - rank)
    };
    egui::Vec2::new(sx as f32 * square_size, sy as f32 * square_size)
}

/// Square background color (Lichess-style).
/// a1 is a dark square: (file + rank) % 2 == 0 → dark.
fn square_color(file: u8, rank: u8) -> egui::Color32 {
    if (file + rank) % 2 == 0 {
        egui::Color32::from_rgb(181, 136, 99) // Dark squares (#b58863)
    } else {
        egui::Color32::from_rgb(240, 217, 181) // Light squares (#f0d9b5)
    }
}

/// Highlight overlay colors.
fn highlight_color(highlight_type: HighlightType) -> egui::Color32 {
    match highlight_type {
        HighlightType::Selected => egui::Color32::from_rgba_unmultiplied(255, 255, 0, 100),
        HighlightType::LegalMove => egui::Color32::from_rgba_unmultiplied(0, 200, 0, 90),
        HighlightType::Capture => egui::Color32::from_rgba_unmultiplied(255, 80, 0, 110),
    }
}

/// Types of square highlights.
#[derive(Debug, Clone, Copy)]
enum HighlightType {
    Selected,
    LegalMove,
    Capture,
}

/// Unicode chess piece symbols.
fn piece_symbol(piece_type: PieceType, color: PieceColor) -> &'static str {
    match (piece_type, color) {
        (PieceType::King, PieceColor::White) => "♔",
        (PieceType::Queen, PieceColor::White) => "♕",
        (PieceType::Rook, PieceColor::White) => "♖",
        (PieceType::Bishop, PieceColor::White) => "♗",
        (PieceType::Knight, PieceColor::White) => "♘",
        (PieceType::Pawn, PieceColor::White) => "♙",
        (PieceType::King, PieceColor::Black) => "♚",
        (PieceType::Queen, PieceColor::Black) => "♛",
        (PieceType::Rook, PieceColor::Black) => "♜",
        (PieceType::Bishop, PieceColor::Black) => "♝",
        (PieceType::Knight, PieceColor::Black) => "♞",
        (PieceType::Pawn, PieceColor::Black) => "♟",
    }
}

/// Main 2D board rendering system.
pub fn render_2d_board(
    mut input_params: InputSystemParams,
    mut contexts: bevy_egui::EguiContexts,
    sprite_handles: Option<Res<crate::rendering::pieces::PieceSpriteHandles>>,
    _game_phase: Res<crate::game::resources::CurrentGamePhase>,
    ai_config: Res<crate::game::ai::ChessAIResource>,
    #[cfg(feature = "solana")] _solana_profile: Option<Res<crate::multiplayer::solana::addon::SolanaProfile>>,
    #[cfg(feature = "solana")] _competitive_match: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
    view_mode: Res<ViewMode>,
    game_mode: Res<GameMode>,
    players: Res<Players>,
    current_turn: Res<CurrentTurn>,
    _hud_visibility: Res<crate::ui::game::game_ui::InGameHudVisibility>,
    fading_captures: Query<&FadingCapture>,
) {
    if *view_mode != ViewMode::Standard2D {
        return;
    }

    // Pre-calculate texture IDs for piece sprites to avoid borrow conflicts with contexts.ctx_mut()
    let mut texture_map: HashMap<(PieceType, PieceColor), egui::TextureId> = HashMap::new();
    if let Some(handles) = &sprite_handles {
        for pt in [PieceType::Pawn, PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen, PieceType::King] {
            for pc in [PieceColor::White, PieceColor::Black] {
                let handle = handles.get(pt, pc);
                if let Some(id) = contexts.image_id(&handle) {
                    texture_map.insert((pt, pc), id);
                }
            }
        }
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let black_view = get_is_black_view(&players, &current_turn, *game_mode);
    let is_human = is_human_turn(&input_params);
    let game_over = input_params.game_over.is_game_over();
    let in_check = input_params.engine.is_check();
    let check_color = input_params.engine.current_turn;

    // Collect active capture flashes: board square → animation progress (0..1).
    // initial_pos.x = file, initial_pos.z = rank (board coord formula).
    let capture_flashes: HashMap<(u8, u8), f32> = fading_captures
        .iter()
        .map(|fc| {
            let file = fc.initial_pos.x.round() as u8;
            let rank = fc.initial_pos.z.round() as u8;
            (file, rank, fc.timer.fraction())
        })
        .filter(|(f, r, _)| *f < 8 && *r < 8)
        .map(|(f, r, prog)| ((f, r), prog))
        .collect();

    // Collect piece positions before the egui closure (read-only borrow).
    let piece_map: HashMap<(u8, u8), (PieceType, PieceColor, Entity)> = {
        let q = input_params.pieces.p1();
        q.iter()
            .map(|(e, p, _, _)| ((p.x, p.y), (p.piece_type, p.color, e)))
            .collect()
    };

    let selected_pos = input_params.selection.selected_position;
    let legal_moves = input_params.selection.possible_moves.clone();
    let is_selected = input_params.selection.is_selected();

    let mut clicked_square: Option<(u8, u8)> = None;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(30, 30, 35)))
        .show(ctx, |ui| {
            let available = ui.available_size();
            let board_size = available.x.min(available.y) * 0.88;
            let square_size = board_size / 8.0;

            let x_offset = (available.x - board_size) / 2.0;
            let y_offset = (available.y - board_size) / 2.0;

            ui.add_space(y_offset);
            ui.horizontal(|ui| {
                ui.add_space(x_offset);

                // Allocate board space for layout only; interaction is per-square below.
                let (board_rect, _) = ui.allocate_exact_size(
                    egui::Vec2::splat(board_size),
                    egui::Sense::hover(),
                );

                let painter = ui.painter_at(board_rect);

                for rank in 0..8u8 {
                    for file in 0..8u8 {
                        let offset = board_to_screen(file, rank, black_view, square_size);
                        let sq_rect = egui::Rect::from_min_size(
                            board_rect.min + offset,
                            egui::Vec2::splat(square_size),
                        );

                        painter.rect_filled(sq_rect, 0.0, square_color(file, rank));

                        if Some((file, rank)) == selected_pos {
                            painter.rect_filled(
                                sq_rect,
                                0.0,
                                highlight_color(HighlightType::Selected),
                            );
                        }

                        if legal_moves.contains(&(file, rank)) {
                            if piece_map.contains_key(&(file, rank)) {
                                painter.rect_filled(
                                    sq_rect,
                                    0.0,
                                    highlight_color(HighlightType::Capture),
                                );
                            } else {
                                painter.circle_filled(
                                    sq_rect.center(),
                                    square_size * 0.15,
                                    highlight_color(HighlightType::LegalMove),
                                );
                            }
                        }

                        // Capture flash: pulsing ring that fades out with the
                        // 3D piece animation so both modes feel in sync.
                        if let Some(&prog) = capture_flashes.get(&(file, rank)) {
                            // opacity peaks early (t=0.1) then fades to zero.
                            let alpha = ((1.0 - prog) * 220.0) as u8;
                            // Ring radius shrinks inward as animation progresses.
                            let ring_r = square_size * 0.5 * (1.0 - prog * 0.4);
                            painter.circle_stroke(
                                sq_rect.center(),
                                ring_r,
                                egui::Stroke::new(
                                    3.0,
                                    egui::Color32::from_rgba_unmultiplied(255, 80, 20, alpha),
                                ),
                            );
                            // Inner fill flash at start of animation.
                            if prog < 0.25 {
                                let fill_alpha = ((1.0 - prog / 0.25) * 80.0) as u8;
                                painter.rect_filled(
                                    sq_rect,
                                    0.0,
                                    egui::Color32::from_rgba_unmultiplied(255, 100, 20, fill_alpha),
                                );
                            }
                        }

                        if let Some((pt, pc, _)) = piece_map.get(&(file, rank)) {
                            let mut piece_drawn = false;

                            // Try to draw sprite first
                            if let Some(id) = texture_map.get(&(*pt, *pc)) {
                                painter.image(
                                    *id,
                                    sq_rect.shrink(square_size * 0.1),
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                                piece_drawn = true;
                            }

                            // Fallback to Unicode if sprite not available
                            if !piece_drawn {
                                let symbol = piece_symbol(*pt, *pc);
                                let font_size = square_size * 0.72;

                                let shadow_col = if *pc == PieceColor::White {
                                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160)
                                } else {
                                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 80)
                                };
                                painter.text(
                                    sq_rect.center() + egui::Vec2::new(1.5, 1.5),
                                    egui::Align2::CENTER_CENTER,
                                    symbol,
                                    egui::FontId::proportional(font_size),
                                    shadow_col,
                                );

                                let piece_col = if *pc == PieceColor::White {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::from_rgb(18, 18, 18)
                                };
                                painter.text(
                                    sq_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    symbol,
                                    egui::FontId::proportional(font_size),
                                    piece_col,
                                );
                            }
                        }

                        // Per-square click detection — reliable, no pointer-position math needed.
                        if is_human && !game_over {
                            let sq_id = egui::Id::new("board_sq")
                                .with(file as u32 * 8 + rank as u32);
                            let sq_resp = ui.interact(sq_rect, sq_id, egui::Sense::click());
                            if sq_resp.clicked() {
                                clicked_square = Some((file, rank));
                            }
                        }
                    }
                }

                // Coordinate labels
                for i in 0..8u8 {
                    let file_label = (b'a' + i) as char;
                    let rank_label = (i + 1).to_string();

                    let file_offset = board_to_screen(i, 0, black_view, square_size);
                    painter.text(
                        egui::Pos2::new(
                            board_rect.min.x + file_offset.x + square_size / 2.0,
                            board_rect.max.y + 5.0,
                        ),
                        egui::Align2::CENTER_TOP,
                        file_label.to_string(),
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgb(160, 160, 160),
                    );

                    let rank_offset = board_to_screen(0, i, black_view, square_size);
                    painter.text(
                        egui::Pos2::new(
                            board_rect.min.x - 10.0,
                            board_rect.min.y + rank_offset.y + square_size / 2.0,
                        ),
                        egui::Align2::RIGHT_CENTER,
                        rank_label,
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgb(160, 160, 160),
                    );
                }

                // ── Check notification badge ──────────────────────────────────
                if in_check && !game_over {
                    let label = match check_color {
                        PieceColor::White => "  White is in Check!",
                        PieceColor::Black => "  Black is in Check!",
                    };
                    let badge_w = 200.0_f32;
                    let badge_h = 32.0_f32;
                    let badge_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(
                            board_rect.center().x - badge_w / 2.0,
                            board_rect.min.y + 8.0,
                        ),
                        egui::Vec2::new(badge_w, badge_h),
                    );
                    painter.rect_filled(
                        badge_rect,
                        6.0,
                        egui::Color32::from_rgba_unmultiplied(200, 40, 40, 235),
                    );
                    painter.text(
                        badge_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(13.5),
                        egui::Color32::WHITE,
                    );
                }
            });
        });

    if game_over || !is_human {
        return;
    }

    let Some((cf, cr)) = clicked_square else {
        return;
    };

    let target = (cf, cr);

    if is_selected {
        if legal_moves.contains(&target) {
            let capture_info = {
                let q = input_params.pieces.p1();
                q.iter()
                    .find(|(_, p, _, _)| p.x == cf && p.y == cr)
                    .map(|(e, p, _, _)| CapturedTarget {
                        entity: e,
                        piece_type: p.piece_type,
                        color: p.color,
                    })
            };
            try_move_sequence(&mut input_params, target, capture_info, "2d_board");
        } else {
            let piece_at = {
                let q = input_params.pieces.p1();
                q.iter()
                    .find(|(_, p, _, _)| p.x == cf && p.y == cr)
                    .map(|(e, p, _, _)| (e, *p))
            };
            match piece_at {
                Some((entity, piece)) if can_move_color(&input_params, piece.color) => {
                    try_select_piece(&mut input_params, entity, piece, true);
                }
                _ => clear_selection_state(
                    &mut input_params.commands,
                    &mut input_params.selection,
                    &input_params.selected_pieces,
                ),
            }
        }
    } else {
        let piece_at = {
            let q = input_params.pieces.p1();
            q.iter()
                .find(|(_, p, _, _)| p.x == cf && p.y == cr)
                .map(|(e, p, _, _)| (e, *p))
        };
        if let Some((entity, piece)) = piece_at {
            if can_move_color(&input_params, piece.color) {
                try_select_piece(&mut input_params, entity, piece, true);
            }
        }
    }
}

