//! The 2D board: draws the squares, pieces and overlays with egui, records what
//! the player did with the pointer and keyboard, then applies it as selections,
//! moves and premoves through the same input helpers the 3D board uses.

use super::*;
use crate::rendering::pieces::Piece;

pub fn render_2d_board(
    mut input_params: InputSystemParams,
    mut contexts: bevy_egui::EguiContexts,
    sprite_handles: Option<Res<crate::rendering::pieces::PieceSpriteHandles>>,
    view_mode: Res<ViewMode>,
    game_mode: Res<GameMode>,
    players: Res<Players>,
    current_turn: Res<CurrentTurn>,
    fading_captures: Query<&FadingCapture>,
    theme: Res<Board2DTheme>,
    mut extras: Board2DExtras,
    mut board_layout: ResMut<Board2DLayout>,
) {
    if *view_mode != ViewMode::Standard2D {
        return;
    }

    // Tick piece animation — independent of board layout.
    if extras.anim.active {
        extras.anim.elapsed += extras.time.delta_secs();
        if extras.anim.elapsed >= PieceAnim2D::DURATION {
            extras.anim.active = false;
        }
    }

    // Resolved before ctx_mut() borrows the contexts.
    let textures = piece_textures(&mut contexts, sprite_handles.as_deref());
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let view = BoardView::gather(
        &mut input_params,
        &players,
        &current_turn,
        *game_mode,
        *theme,
        &fading_captures,
        &extras,
        textures,
    );
    let mut input = BoardInput::default();

    // Match the normal board footprint so switching 2D/3D views preserves alignment.
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(
        &mut crate::ui::egui_ext::viewport_ui(ctx, "game_2d_root"),
        |ui| draw_board_with_bars(ui, &view, &mut extras, &mut board_layout, &mut input),
    );

    apply_input(&mut input_params, &mut extras, &current_turn, &view, input);
}

/// The board's place on screen and orientation.
#[derive(Clone, Copy)]
struct BoardGeom {
    rect: egui::Rect,
    square: f32,
    black_view: bool,
}

impl BoardGeom {
    fn square_rect(&self, file: u8, rank: u8) -> egui::Rect {
        egui::Rect::from_min_size(
            self.rect.min + board_to_screen(file, rank, self.black_view, self.square),
            egui::Vec2::splat(self.square),
        )
    }

    fn square_center(&self, file: u8, rank: u8) -> egui::Pos2 {
        self.square_rect(file, rank).center()
    }

    /// The square under a screen position, if it's on the board.
    fn square_at(&self, pos: egui::Pos2) -> Option<(u8, u8)> {
        let rel = pos - self.rect.min;
        let size = self.square * 8.0;
        if rel.x < 0.0 || rel.y < 0.0 || rel.x >= size || rel.y >= size {
            return None;
        }
        let col = (rel.x / self.square) as u8;
        let row = (rel.y / self.square) as u8;
        Some(if self.black_view {
            (7 - col, row)
        } else {
            (col, 7 - row)
        })
    }
}

/// Everything the board shows this frame, gathered before drawing.
struct BoardView {
    black_view: bool,
    is_human: bool,
    game_over: bool,
    piece_alpha: u8,
    in_check: bool,
    check_color: PieceColor,
    last_move: Option<((u8, u8), (u8, u8))>,
    theme: Board2DTheme,
    capture_flashes: HashMap<(u8, u8), f32>,
    pieces: HashMap<(u8, u8), (PieceType, PieceColor, Entity)>,
    selected: Option<(u8, u8)>,
    legal_moves: Vec<(u8, u8)>,
    is_selected: bool,
    textures: HashMap<(PieceType, PieceColor), egui::TextureId>,
}

impl BoardView {
    #[allow(clippy::too_many_arguments)]
    fn gather(
        input_params: &mut InputSystemParams,
        players: &Players,
        current_turn: &CurrentTurn,
        game_mode: GameMode,
        theme: Board2DTheme,
        fading_captures: &Query<&FadingCapture>,
        extras: &Board2DExtras,
        textures: HashMap<(PieceType, PieceColor), egui::TextureId>,
    ) -> Self {
        // Convert mirrored capture X (7-file) back to the logical file for 2D flashes.
        let capture_flashes = fading_captures
            .iter()
            .map(|fc| {
                let file = 7u8.saturating_sub(fc.initial_pos.x.round() as u8);
                let rank = fc.initial_pos.z.round() as u8;
                (file, rank, fc.timer.fraction())
            })
            .filter(|(f, r, _)| *f < 8 && *r < 8)
            .map(|(f, r, prog)| ((f, r), prog))
            .collect();
        let pieces = input_params
            .pieces
            .p1()
            .iter()
            .map(|(e, p, _, _)| ((p.x, p.y), (p.piece_type, p.color, e)))
            .collect();

        Self {
            black_view: get_is_black_view(players, current_turn, game_mode),
            is_human: is_human_turn(input_params),
            game_over: input_params.game_over.is_game_over(),
            piece_alpha: (extras.board_fade.alpha_mult * 255.0).clamp(0.0, 255.0) as u8,
            in_check: input_params.engine.is_check(),
            // Use the engine's side to move for check; replay and network lag can
            // leave CurrentTurn stale.
            check_color: input_params.engine.side_to_move(),
            last_move: input_params.move_history.moves.last().map(|m| (m.from, m.to)),
            theme,
            capture_flashes,
            pieces,
            selected: input_params.selection.selected_position,
            legal_moves: input_params.selection.possible_moves.clone(),
            is_selected: input_params.selection.is_selected(),
            textures,
        }
    }

    fn draw_piece(&self, painter: &egui::Painter, piece: (PieceType, PieceColor), rect: egui::Rect, tint: egui::Color32) {
        if let Some(id) = self.textures.get(&piece) {
            painter.image(
                *id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                tint,
            );
        }
    }
}

/// What the player did on the board this frame; applied after drawing.
#[derive(Default)]
struct BoardInput {
    clicked: Option<(u8, u8)>,
    promotion: Option<PieceType>,
    drag_started_at: Option<(u8, u8)>,
    drag_released_at: Option<(u8, u8)>,
    drag_cursor: Option<egui::Pos2>,
    premove_click: Option<(u8, u8)>,
}

/// Egui texture ids for every piece sprite.
fn piece_textures(
    contexts: &mut bevy_egui::EguiContexts,
    handles: Option<&crate::rendering::pieces::PieceSpriteHandles>,
) -> HashMap<(PieceType, PieceColor), egui::TextureId> {
    let mut textures = HashMap::new();
    let Some(handles) = handles else {
        return textures;
    };
    for pt in [
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ] {
        for pc in [PieceColor::White, PieceColor::Black] {
            let handle = handles.get(pt, pc);
            let id = contexts.image_id(&handle).unwrap_or_else(|| {
                contexts.add_image(bevy_egui::EguiTextureHandle::Strong(handle.clone()))
            });
            textures.insert((pt, pc), id);
        }
    }
    textures
}

/// Player bars above and below, the board between them.
fn draw_board_with_bars(
    ui: &mut egui::Ui,
    view: &BoardView,
    extras: &mut Board2DExtras,
    board_layout: &mut Board2DLayout,
    input: &mut BoardInput,
) {
    let available = ui.available_size();
    const BAR_GAP: f32 = 8.0;
    const BAR_HEADROOM: f32 = 8.0;
    let bars_h =
        extras.bar_layout.top_h + extras.bar_layout.bottom_h + BAR_HEADROOM * 2.0 + BAR_GAP * 2.0;
    let board_size = available.x.min((available.y - bars_h).max(0.0));
    let x_offset = (available.x - board_size) / 2.0;

    let top_bar = ui.horizontal(|ui| {
        ui.add_space(x_offset);
        ui.vertical(|ui| {
            ui.set_width(board_size);
            crate::ui::game::player_bar::render_player_bar(ui, &extras.player_bars.top);
        });
    });
    extras.bar_layout.top_h = top_bar.response.rect.height();
    ui.add_space(BAR_GAP);
    ui.horizontal(|ui| {
        ui.add_space(x_offset);
        draw_board(ui, board_size, view, extras, board_layout, input);
    });
    ui.add_space(BAR_GAP);
    let bottom_bar = ui.horizontal(|ui| {
        ui.add_space(x_offset);
        ui.vertical(|ui| {
            ui.set_width(board_size);
            crate::ui::game::player_bar::render_player_bar(ui, &extras.player_bars.bottom);
        });
    });
    extras.bar_layout.bottom_h = bottom_bar.response.rect.height();
}

fn draw_board(
    ui: &mut egui::Ui,
    board_size: f32,
    view: &BoardView,
    extras: &mut Board2DExtras,
    board_layout: &mut Board2DLayout,
    input: &mut BoardInput,
) {
    // Allocate board space; right-click-drag draws arrows, left-drag moves pieces.
    let (rect, board_resp) =
        ui.allocate_exact_size(egui::Vec2::splat(board_size), egui::Sense::click_and_drag());
    board_layout.rect = Some(rect);
    let geom = BoardGeom {
        rect,
        square: board_size / 8.0,
        black_view: view.black_view,
    };
    let arrow_kind = arrow_kind(&extras.keyboard);
    handle_board_pointer(&board_resp, geom, arrow_kind, extras, input);

    let painter = ui.painter_at(rect);
    for rank in 0..8u8 {
        for file in 0..8u8 {
            draw_square(ui, &painter, geom, file, rank, view, extras, input);
        }
    }
    draw_moving_pieces(&painter, geom, view, extras);
    draw_promotion_picker(ui, &painter, geom, view, extras, input);
    draw_arrows(&painter, geom, &board_resp, arrow_kind, extras);
    handle_keyboard_focus(view, extras, input);
    draw_eval_bar(&painter, geom, extras);
    draw_check_badge(&painter, geom, view);
}

/// Shift draws orange arrows, Alt blue ones, otherwise green.
fn arrow_kind(keyboard: &ButtonInput<KeyCode>) -> u8 {
    if keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        1
    } else if keyboard.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]) {
        2
    } else {
        0
    }
}

fn arrow_color(kind: u8, alpha: u8) -> egui::Color32 {
    match kind {
        1 => egui::Color32::from_rgba_unmultiplied(255, 140, 0, alpha),
        2 => egui::Color32::from_rgba_unmultiplied(80, 160, 255, alpha),
        _ => egui::Color32::from_rgba_unmultiplied(20, 200, 60, alpha),
    }
}

/// Right-drag draws or clears arrows; left-drag starts, tracks and drops a move.
fn handle_board_pointer(
    board_resp: &egui::Response,
    geom: BoardGeom,
    arrow_kind: u8,
    extras: &mut Board2DExtras,
    input: &mut BoardInput,
) {
    // Right-click drag start
    if board_resp.drag_started_by(egui::PointerButton::Secondary) {
        if let Some(pos) = board_resp.interact_pointer_pos() {
            extras.arrows.drag_from = geom.square_at(pos);
        }
    }
    // Right-click drag release → store arrow
    if board_resp.drag_stopped_by(egui::PointerButton::Secondary) {
        if let Some(from) = extras.arrows.drag_from.take() {
            if let Some(to) = board_resp.interact_pointer_pos().and_then(|p| geom.square_at(p)) {
                if to != from {
                    extras
                        .arrows
                        .arrows
                        .push((from.0, from.1, to.0, to.1, arrow_kind));
                } else {
                    extras.arrows.arrows.clear();
                }
            }
        }
    }
    // Right-click without drag — also clear
    if board_resp.secondary_clicked() && extras.arrows.drag_from.is_none() {
        extras.arrows.arrows.clear();
    }
    // Left-click drag start → drag-to-move
    if board_resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(pos) = board_resp.interact_pointer_pos() {
            input.drag_started_at = geom.square_at(pos);
        }
    }
    // Left-click drag in progress → update cursor
    if extras.drag.dragging {
        if let Some(pos) = board_resp.interact_pointer_pos() {
            input.drag_cursor = Some(pos);
        }
    }
    // Left-click drag release → execute, or cancel when released off the board.
    if board_resp.drag_stopped_by(egui::PointerButton::Primary) && extras.drag.dragging {
        input.drag_released_at = match board_resp.interact_pointer_pos() {
            Some(pos) => geom.square_at(pos),
            None => Some(extras.drag.from),
        };
    }
}

/// One square: colour, highlights, the piece on it, coordinates, focus ring and
/// click detection.
#[allow(clippy::too_many_arguments)]
fn draw_square(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    geom: BoardGeom,
    file: u8,
    rank: u8,
    view: &BoardView,
    extras: &Board2DExtras,
    input: &mut BoardInput,
) {
    let sq = (file, rank);
    let sq_rect = geom.square_rect(file, rank);
    let tint = |a: u8, r: u8, g: u8, b: u8| egui::Color32::from_rgba_unmultiplied(r, g, b, a);

    // Base square color from active theme
    painter.rect_filled(sq_rect, 0.0, view.theme.sq_color(file, rank));

    // Last-move highlight (golden tint, below selection)
    if view.last_move.is_some_and(|(from, to)| sq == from || sq == to) {
        painter.rect_filled(sq_rect, 0.0, tint(90, 205, 210, 100));
    }
    // Premove highlight (cyan tint)
    if extras.premove.from == Some(sq) || extras.premove.to == Some(sq) {
        painter.rect_filled(sq_rect, 0.0, tint(100, 0, 200, 220));
    }
    // Check king tint (red glow on the king in check)
    if view.in_check && !view.game_over {
        if let Some((PieceType::King, pc, _)) = view.pieces.get(&sq) {
            if *pc == view.check_color {
                painter.rect_filled(sq_rect, 0.0, tint(130, 200, 30, 30));
            }
        }
    }
    // Checkmate amber flash on mated king square
    if extras.cm_flash.active && sq == extras.cm_flash.king_sq {
        painter.rect_filled(sq_rect, 0.0, tint(extras.cm_flash.alpha(), 255, 160, 0));
    }
    if Some(sq) == view.selected {
        painter.rect_filled(sq_rect, 0.0, highlight_color(HighlightType::Selected));
    }
    if view.legal_moves.contains(&sq) {
        if view.pieces.contains_key(&sq) {
            painter.rect_filled(sq_rect, 0.0, highlight_color(HighlightType::Capture));
        } else {
            painter.circle_filled(
                sq_rect.center(),
                geom.square * 0.15,
                highlight_color(HighlightType::LegalMove),
            );
        }
    }

    // Capture flash: a ring that shrinks and fades with the 3D capture animation,
    // plus a brief fill at its start.
    if let Some(&prog) = view.capture_flashes.get(&sq) {
        let alpha = ((1.0 - prog) * 220.0) as u8;
        let ring_r = geom.square * 0.5 * (1.0 - prog * 0.4);
        painter.circle_stroke(
            sq_rect.center(),
            ring_r,
            egui::Stroke::new(3.0, tint(alpha, 255, 80, 20)),
        );
        if prog < 0.25 {
            let fill_alpha = ((1.0 - prog / 0.25) * 80.0) as u8;
            painter.rect_filled(sq_rect, 0.0, tint(fill_alpha, 255, 100, 20));
        }
    }

    // The drag source and the animation destination are drawn separately.
    let skip_piece = (extras.drag.dragging && extras.drag.from == sq)
        || (extras.anim.active && extras.anim.to_sq == sq);
    if !extras.settings.blindfold && !skip_piece {
        if let Some((pt, pc, _)) = view.pieces.get(&sq) {
            view.draw_piece(
                painter,
                (*pt, *pc),
                sq_rect,
                tint(view.piece_alpha, 255, 255, 255),
            );
        }
    }

    // In-board corner coordinate labels (Lichess style)
    let is_bottom_row = if view.black_view { rank == 7 } else { rank == 0 };
    let is_left_col = if view.black_view { file == 7 } else { file == 0 };
    let label_color = view.theme.label_color(file, rank);
    let font = egui::FontId::proportional(9.0);
    if is_left_col {
        painter.text(
            sq_rect.min + egui::Vec2::new(2.0, 2.0),
            egui::Align2::LEFT_TOP,
            (rank + 1).to_string(),
            font.clone(),
            label_color,
        );
    }
    if is_bottom_row {
        painter.text(
            sq_rect.max - egui::Vec2::new(2.0, 2.0),
            egui::Align2::RIGHT_BOTTOM,
            ((b'a' + file) as char).to_string(),
            font,
            label_color,
        );
    }

    // Keyboard nav focus ring
    if extras.focus.active && extras.focus.cursor == sq {
        painter.rect_stroke(
            sq_rect,
            0.0,
            egui::Stroke::new(2.5, egui::Color32::from_rgb(0, 210, 240)),
            egui::StrokeKind::Outside,
        );
    }

    let sq_id = egui::Id::new("board_sq").with(file as u32 * 8 + rank as u32);
    if ui.interact(sq_rect, sq_id, egui::Sense::click()).clicked() && !view.game_over {
        if view.is_human {
            input.clicked = Some(sq);
        } else {
            input.premove_click = Some(sq);
        }
    }
}

/// The piece sliding to its new square, and the piece under the cursor while
/// dragging.
fn draw_moving_pieces(
    painter: &egui::Painter,
    geom: BoardGeom,
    view: &BoardView,
    extras: &mut Board2DExtras,
) {
    if extras.settings.blindfold {
        return;
    }
    let size = egui::Vec2::splat(geom.square);
    if extras.anim.active {
        if !extras.anim.pixels_ready {
            extras.anim.from_px = geom.square_center(extras.anim.from_sq.0, extras.anim.from_sq.1);
            extras.anim.to_px = geom.square_center(extras.anim.to_sq.0, extras.anim.to_sq.1);
            extras.anim.pixels_ready = true;
            // Fill piece identity from the board (the piece is already at to_sq).
            if extras.anim.piece.is_none() {
                if let Some(&(pt, pc, _)) = view.pieces.get(&extras.anim.to_sq) {
                    extras.anim.piece = Some((pt, pc));
                }
            }
        }
        if let Some(piece) = extras.anim.piece {
            let rect = egui::Rect::from_center_size(extras.anim.lerped_pos(), size);
            view.draw_piece(painter, piece, rect, egui::Color32::WHITE);
        }
    }
    if extras.drag.dragging {
        if let Some(piece) = extras.drag.piece {
            let rect = egui::Rect::from_center_size(extras.drag.cursor_pos, size);
            view.draw_piece(painter, piece, rect, egui::Color32::WHITE);
        }
    }
}

/// Queen, rook, bishop and knight buttons over a dimmed board while a pawn
/// waits to promote.
fn draw_promotion_picker(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    geom: BoardGeom,
    view: &BoardView,
    extras: &Board2DExtras,
    input: &mut BoardInput,
) {
    if !extras.promotion.is_active() {
        return;
    }
    let (Some((pf, pr)), Some(pcolor)) = (extras.promotion.position, extras.promotion.color)
    else {
        return;
    };
    painter.rect_filled(
        geom.rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
    );
    // The column runs from the promotion square toward the board centre.
    let toward_lower_ranks = pr == 7;
    let choices = [
        PieceType::Queen,
        PieceType::Rook,
        PieceType::Bishop,
        PieceType::Knight,
    ];
    for (idx, &pt) in choices.iter().enumerate() {
        let btn_rank = if toward_lower_ranks {
            pr.saturating_sub(idx as u8)
        } else {
            pr.saturating_add(idx as u8).min(7)
        };
        let btn_rect = geom.square_rect(pf, btn_rank);
        let bg = if idx % 2 == 0 {
            egui::Color32::from_rgb(240, 217, 181)
        } else {
            egui::Color32::from_rgb(181, 136, 99)
        };
        painter.rect_filled(btn_rect, 4.0, bg);
        view.draw_piece(painter, (pt, pcolor), btn_rect, egui::Color32::WHITE);
        let btn_resp = ui.interact(
            btn_rect,
            egui::Id::new("promo_btn").with(idx as u32),
            egui::Sense::click(),
        );
        if btn_resp.clicked() {
            input.promotion = Some(pt);
        }
        if btn_resp.hovered() {
            painter.rect_stroke(
                btn_rect,
                4.0,
                egui::Stroke::new(2.5, egui::Color32::from_rgb(255, 220, 80)),
                egui::StrokeKind::Outside,
            );
        }
    }
}

/// Saved arrows, and the one being drawn with a right-drag.
fn draw_arrows(
    painter: &egui::Painter,
    geom: BoardGeom,
    board_resp: &egui::Response,
    arrow_kind: u8,
    extras: &Board2DExtras,
) {
    for &(ff, fr, tf, tr, kind) in &extras.arrows.arrows {
        let from = geom.square_center(ff, fr);
        let to = geom.square_center(tf, tr);
        painter.arrow(
            from,
            to - from,
            egui::Stroke::new(geom.square * 0.12, arrow_color(kind, 180)),
        );
    }
    if let (Some((ff, fr)), Some(cursor)) =
        (extras.arrows.drag_from, board_resp.interact_pointer_pos())
    {
        let from = geom.square_center(ff, fr);
        painter.arrow(
            from,
            cursor - from,
            egui::Stroke::new(geom.square * 0.10, arrow_color(arrow_kind, 120)),
        );
    }
}

/// Tab toggles keyboard navigation; arrows move the focus ring (mirrored for
/// Black), Enter or Space acts on the focused square.
fn handle_keyboard_focus(view: &BoardView, extras: &mut Board2DExtras, input: &mut BoardInput) {
    if extras.keyboard.just_pressed(KeyCode::Tab) {
        extras.focus.active = !extras.focus.active;
    }
    if !extras.focus.active || view.game_over {
        return;
    }
    let (mut f, mut r) = extras.focus.cursor;
    let keys = &extras.keyboard;
    if keys.just_pressed(KeyCode::ArrowRight) {
        f = if view.black_view {
            f.saturating_sub(1)
        } else {
            (f + 1).min(7)
        };
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        f = if view.black_view {
            (f + 1).min(7)
        } else {
            f.saturating_sub(1)
        };
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        r = (r + 1).min(7);
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        r = r.saturating_sub(1);
    }
    let act = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
    extras.focus.cursor = (f, r);
    if act {
        if view.is_human {
            input.clicked = Some((f, r));
        } else {
            input.premove_click = Some((f, r));
        }
    }
}

/// The evaluation bar to the right of the board: white's share from the bottom,
/// with the score once it's beyond ±0.2.
fn draw_eval_bar(painter: &egui::Painter, geom: BoardGeom, extras: &Board2DExtras) {
    if !extras.eval_bar.visible {
        return;
    }
    let board_size = geom.square * 8.0;
    let bar_w = 14.0_f32;
    let bar_x = geom.rect.max.x + 8.0;
    let bar_rect = egui::Rect::from_min_size(
        egui::Pos2::new(bar_x, geom.rect.min.y),
        egui::Vec2::new(bar_w, board_size),
    );
    painter.rect_filled(bar_rect, 3.0, egui::Color32::from_rgb(40, 40, 40));
    let white_h = board_size * extras.eval_bar.white_fraction();
    let white_rect = egui::Rect::from_min_size(
        egui::Pos2::new(bar_x, geom.rect.max.y - white_h),
        egui::Vec2::new(bar_w, white_h),
    );
    painter.rect_filled(white_rect, 3.0, egui::Color32::from_rgb(230, 230, 230));

    let abs_cp = extras.eval_bar.score.unsigned_abs();
    if abs_cp > 20 {
        let label = if abs_cp >= 1000 {
            format!("M{}", abs_cp / 100)
        } else {
            format!("{:.1}", abs_cp as f32 / 100.0)
        };
        let label_y = if extras.eval_bar.score > 0 {
            white_rect.min.y + 3.0
        } else {
            white_rect.min.y - 14.0
        };
        painter.text(
            egui::Pos2::new(bar_rect.center().x, label_y),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::proportional(9.0),
            egui::Color32::from_gray(120),
        );
    }
}

/// A red "in check" badge along the top of the board.
fn draw_check_badge(painter: &egui::Painter, geom: BoardGeom, view: &BoardView) {
    if !view.in_check || view.game_over {
        return;
    }
    let label = match view.check_color {
        PieceColor::White => "  White is in Check!",
        PieceColor::Black => "  Black is in Check!",
    };
    let (badge_w, badge_h) = (200.0_f32, 32.0_f32);
    let badge_rect = egui::Rect::from_min_size(
        egui::Pos2::new(
            geom.rect.center().x - badge_w / 2.0,
            geom.rect.min.y + 8.0,
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

/// The piece on a square.
fn piece_on(input_params: &mut InputSystemParams, (file, rank): (u8, u8)) -> Option<(Entity, Piece)> {
    input_params
        .pieces
        .p1()
        .iter()
        .find(|(_, p, _, _)| p.x == file && p.y == rank)
        .map(|(e, p, _, _)| (e, *p))
}

/// The piece a move to `sq` would capture.
fn capture_on(input_params: &mut InputSystemParams, sq: (u8, u8)) -> Option<CapturedTarget> {
    piece_on(input_params, sq).map(|(entity, p)| CapturedTarget {
        entity,
        piece_type: p.piece_type,
        color: p.color,
    })
}

/// Applies what the player did on the board this frame: a promotion choice,
/// drag-to-move, premoves (queued on the opponent's turn, fired on ours) and
/// click-to-select / click-to-move.
fn apply_input(
    input_params: &mut InputSystemParams,
    extras: &mut Board2DExtras,
    current_turn: &CurrentTurn,
    view: &BoardView,
    input: BoardInput,
) {
    if let (Some(pt), Some(entity), Some(position)) = (
        input.promotion,
        extras.promotion.pawn_entity,
        extras.promotion.position,
    ) {
        extras
            .promotion_writer
            .write(crate::game::resources::PromotionSelected {
                entity,
                position,
                promoted_to: pt,
            });
    }
    if let Some(pos) = input.drag_cursor {
        extras.drag.cursor_pos = pos;
    }
    if view.game_over {
        return;
    }

    if let Some(sq) = input.drag_started_at {
        if let Some((entity, piece)) = piece_on(input_params, sq) {
            if view.is_human && can_move_color(input_params, piece.color) {
                extras.drag.dragging = true;
                extras.drag.from = sq;
                extras.drag.piece = Some((piece.piece_type, piece.color));
                try_select_piece(input_params, entity, piece, true);
            }
        }
    }

    if let Some(to_sq) = input.drag_released_at {
        if extras.drag.dragging {
            extras.drag.dragging = false;
            extras.drag.piece = None;
            if to_sq != extras.drag.from && view.legal_moves.contains(&to_sq) {
                let capture = capture_on(input_params, to_sq);
                try_move_sequence(input_params, to_sq, capture, "2d_drag");
            } else {
                clear_selection_state(
                    &mut input_params.commands,
                    &mut input_params.selection,
                    &input_params.selected_pieces,
                );
            }
        }
    }

    if !view.is_human {
        if let Some(sq) = input.premove_click {
            queue_premove(input_params, extras, current_turn, sq);
        }
        return;
    }
    if fire_premove(input_params, extras) {
        return; // the premove was the move — don't also process a click
    }
    if let Some(target) = input.clicked {
        click_square(input_params, extras, view, target);
    }
}

/// Premove clicks on the opponent's turn: own piece, then target; clicking the
/// same square or a third time cancels.
fn queue_premove(
    input_params: &mut InputSystemParams,
    extras: &mut Board2DExtras,
    current_turn: &CurrentTurn,
    sq: (u8, u8),
) {
    if extras.premove.from.is_none() {
        // The local player's color is the opponent of whoever is currently moving.
        let local_color = match current_turn.color {
            PieceColor::White => PieceColor::Black,
            PieceColor::Black => PieceColor::White,
        };
        if piece_on(input_params, sq).is_some_and(|(_, p)| p.color == local_color) {
            extras.premove.from = Some(sq);
        }
    } else if extras.premove.to.is_none() && Some(sq) != extras.premove.from {
        extras.premove.to = Some(sq);
    } else {
        extras.premove.clear();
    }
}

/// Plays a queued premove now that it's our turn, if it's still legal. Returns
/// whether a move was made.
fn fire_premove(input_params: &mut InputSystemParams, extras: &mut Board2DExtras) -> bool {
    let (Some(from), Some(to)) = (extras.premove.from, extras.premove.to) else {
        return false;
    };
    extras.premove.clear();
    let Some((entity, piece)) = piece_on(input_params, from) else {
        return false;
    };
    if !can_move_color(input_params, piece.color) {
        return false;
    }
    try_select_piece(input_params, entity, piece, true);
    if !input_params.selection.possible_moves.contains(&to) {
        return false;
    }
    let capture = capture_on(input_params, to);
    try_move_sequence(input_params, to, capture, "2d_premove");
    true
}

/// Click-to-move with a piece selected, otherwise click-to-select; an illegal
/// target plays the illegal-move sound and clears the selection.
fn click_square(
    input_params: &mut InputSystemParams,
    extras: &Board2DExtras,
    view: &BoardView,
    target: (u8, u8),
) {
    if view.is_selected && view.legal_moves.contains(&target) {
        let capture = capture_on(input_params, target);
        try_move_sequence(input_params, target, capture, "2d_board");
        return;
    }
    match piece_on(input_params, target) {
        Some((entity, piece)) if can_move_color(input_params, piece.color) => {
            try_select_piece(input_params, entity, piece, true);
        }
        _ if view.is_selected => {
            if let Some(sounds) = &extras.sounds {
                if !extras.settings.muted {
                    crate::game::resources::sounds::play_sfx(
                        &mut input_params.commands,
                        sounds.illegal.clone(),
                    );
                }
            }
            clear_selection_state(
                &mut input_params.commands,
                &mut input_params.selection,
                &input_params.selected_pieces,
            );
        }
        _ => {}
    }
}
