//! The 2D board: draws the squares, pieces and overlays with egui, records what
//! the player did with the pointer and keyboard, then applies it as selections,
//! moves and premoves through the same input helpers the 3D board uses. Pieces
//! mid-move are drawn from the shared `motion` timeline, which is advanced in
//! `Update`, never here.

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
    motions: Query<&PieceMotion>,
    theme: Res<Board2DTheme>,
    mut extras: Board2DExtras,
    mut board_layout: ResMut<Board2DLayout>,
) {
    if *view_mode != ViewMode::Standard2D {
        return;
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
        &motions,
        &extras,
        textures,
    );
    if extras.drag.dragging
        && view.pieces.get(&extras.drag.from).map(|p| (p.0, p.1)) != extras.drag.piece
    {
        extras.drag.dragging = false;
        extras.drag.piece = None;
    }
    let mut input = BoardInput::default();

    // Match the normal board footprint so switching 2D/3D views preserves alignment.
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(
        &mut crate::ui::egui_ext::viewport_ui(ctx, "game_2d_root"),
        |ui| draw_board_with_bars(ui, &view, &mut extras, &mut board_layout, &mut input),
    );

    apply_input(&mut input_params, &mut extras, &view, input);
}

/// The board's place on screen and orientation.
#[derive(Clone, Copy)]
pub(crate) struct BoardGeom {
    pub(crate) rect: egui::Rect,
    pub(crate) square: f32,
    pub(crate) black_view: bool,
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

    /// Screen position of a possibly fractional board position, recomputed from
    /// the current geometry on every draw so resizing and flipping just work.
    pub(crate) fn point_center(&self, position: Vec2) -> egui::Pos2 {
        let (sx, sy) = if self.black_view {
            (7.0 - position.x, position.y)
        } else {
            (position.x, 7.0 - position.y)
        };
        self.rect.min + egui::Vec2::new((sx + 0.5) * self.square, (sy + 0.5) * self.square)
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

/// How much a piece grows per unit of lift.
const LIFT_SCALE: f32 = 0.25;

/// One sample of a piece's [`PieceMotion`], read without advancing it.
struct MovingPiece {
    position: Vec2,
    lift: f32,
    piece: (PieceType, PieceColor),
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
    /// Pieces mid-move: drawn at the sampled position, not on their square.
    moving: HashMap<Entity, MovingPiece>,
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
        motions: &Query<&PieceMotion>,
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
        let moving = input_params
            .pieces
            .p1()
            .iter()
            .filter_map(|(e, p, _, _)| {
                // Captured pieces are parked off the board while they fade.
                if p.x >= 8 || p.y >= 8 {
                    return None;
                }
                let motion = motions.get(e).ok()?;
                let sample = motion.tx.sample();
                Some((
                    e,
                    MovingPiece {
                        position: sample.position2(motion.tx.from, motion.tx.to),
                        lift: sample.lift,
                        piece: (motion.drawn_type(p.piece_type), p.color),
                    },
                ))
            })
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
            last_move: input_params
                .move_history
                .moves
                .last()
                .map(|m| (m.from, m.to)),
            theme,
            capture_flashes,
            pieces,
            moving,
            selected: input_params.selection.selected_position,
            legal_moves: input_params.selection.possible_moves.clone(),
            is_selected: input_params.selection.is_selected(),
            textures,
        }
    }

    fn draw_piece(
        &self,
        painter: &egui::Painter,
        piece: (PieceType, PieceColor),
        rect: egui::Rect,
        tint: egui::Color32,
    ) {
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
    draw_moving_pieces(&painter, geom, view, &*extras);
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
            if let Some(to) = board_resp
                .interact_pointer_pos()
                .and_then(|p| geom.square_at(p))
            {
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
    if view
        .last_move
        .is_some_and(|(from, to)| sq == from || sq == to)
    {
        painter.rect_filled(sq_rect, 0.0, tint(90, 205, 210, 100));
    }
    // Premove highlight (cyan tint)
    if extras.premove.marks(sq) {
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

    // The drag source and any piece mid-move are drawn separately; a moving
    // piece already occupies this square on the board, so skip it here.
    let skip_piece = extras.drag.dragging && extras.drag.from == sq;
    if !extras.settings.blindfold && !skip_piece {
        if let Some((pt, pc, entity)) = view.pieces.get(&sq) {
            if !view.moving.contains_key(entity) {
                view.draw_piece(
                    painter,
                    (*pt, *pc),
                    sq_rect,
                    tint(view.piece_alpha, 255, 255, 255),
                );
            }
        }
    }

    // In-board corner coordinate labels (Lichess style)
    let is_bottom_row = if view.black_view {
        rank == 7
    } else {
        rank == 0
    };
    let is_left_col = if view.black_view {
        file == 7
    } else {
        file == 0
    };
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

/// Pieces sliding to their new squares, and the piece under the cursor while
/// dragging.
fn draw_moving_pieces(
    painter: &egui::Painter,
    geom: BoardGeom,
    view: &BoardView,
    extras: &Board2DExtras,
) {
    if extras.settings.blindfold {
        return;
    }
    let size = egui::Vec2::splat(geom.square);
    // Raised pieces draw last so they pass over the ones still on the board.
    let dragged = extras
        .drag
        .dragging
        .then(|| view.pieces.get(&extras.drag.from).map(|p| p.2))
        .flatten();
    let mut moving: Vec<&MovingPiece> = view
        .moving
        .iter()
        .filter(|(entity, _)| Some(**entity) != dragged)
        .map(|(_, piece)| piece)
        .collect();
    moving.sort_by(|a, b| a.lift.total_cmp(&b.lift));
    for piece in moving {
        // The same lift the 3D view shows, as a slight enlargement.
        let rect = egui::Rect::from_center_size(
            geom.point_center(piece.position),
            size * (1.0 + piece.lift * LIFT_SCALE),
        );
        view.draw_piece(painter, piece.piece, rect, egui::Color32::WHITE);
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
    let (Some((pf, pr)), Some(pcolor)) = (extras.promotion.position, extras.promotion.color) else {
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
    if extras.eval_bar.status != EvaluationStatus::Ready {
        let label = if extras.eval_bar.status == EvaluationStatus::Pending {
            "..."
        } else {
            "?"
        };
        painter.text(
            bar_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.0),
            egui::Color32::GRAY,
        );
        return;
    }
    let white_h = board_size * extras.eval_bar.white_fraction();
    let white_rect = egui::Rect::from_min_size(
        egui::Pos2::new(bar_x, geom.rect.max.y - white_h),
        egui::Vec2::new(bar_w, white_h),
    );
    painter.rect_filled(white_rect, 3.0, egui::Color32::from_rgb(230, 230, 230));

    let abs_cp = extras.eval_bar.score.unsigned_abs();
    if abs_cp > 20 || extras.eval_bar.mate.is_some() {
        let label = if let Some(mate) = extras.eval_bar.mate {
            format!("M{}", mate.unsigned_abs())
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
        egui::Pos2::new(geom.rect.center().x - badge_w / 2.0, geom.rect.min.y + 8.0),
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
fn piece_on(
    input_params: &mut InputSystemParams,
    (file, rank): (u8, u8),
) -> Option<(Entity, Piece)> {
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
/// drag-to-move, premoves (queued here on the opponent's turn, played by
/// `input::board::fire_premove` on ours) and click-to-select / click-to-move.
fn apply_input(
    input_params: &mut InputSystemParams,
    extras: &mut Board2DExtras,
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
                queue_board_move(input_params, to_sq, capture, "2d_drag");
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
            queue_premove_click(input_params, &mut extras.premove, &extras.epoch, sq);
        }
        return;
    }
    if let Some(target) = input.clicked {
        click_square(input_params, extras, view, target);
    }
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
        queue_board_move(input_params, target, capture, "2d_board");
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

#[cfg(test)]
mod projection_tests {
    use super::*;
    use crate::motion::{MotionStyle, MoveTransaction};

    #[test]
    fn egui_paints_each_moving_piece_once_including_creation_and_drag_frames() -> Result<(), String>
    {
        use bevy::ecs::system::SystemState;
        let mut world = World::new();
        world.init_resource::<EvalBarState>();
        world.init_resource::<BoardArrows>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<crate::core::GameSettings>();
        world.init_resource::<DragState2D>();
        world.init_resource::<crate::game::resources::premove::Premove>();
        world.init_resource::<crate::motion::BoardEpoch>();
        world.init_resource::<crate::game::resources::PendingPromotion>();
        world.init_resource::<Messages<crate::game::resources::PromotionSelected>>();
        world.init_resource::<BoardFocus>();
        world.init_resource::<CheckmateFlashState>();
        world.init_resource::<BoardFadeState>();
        world.init_resource::<crate::ui::game::player_bar::PlayerBarsCache>();
        world.init_resource::<crate::ui::game::player_bar::PlayerBarLayoutCache>();
        let knight = world.spawn_empty().id();
        let rook = world.spawn_empty().id();
        let mut state = SystemState::<Board2DExtras>::new(&mut world);
        for elapsed in [0.0, 0.16, 0.4] {
            for dragged in [false, true] {
                let mut extras = state
                    .get_mut(&mut world)
                    .map_err(|error| error.to_string())?;
                extras.drag.dragging = dragged;
                extras.drag.from = (5, 2);
                extras.drag.piece = Some((PieceType::Knight, PieceColor::White));
                extras.drag.cursor_pos = egui::pos2(300.0, 300.0);
                let mut moving = HashMap::new();
                for (entity, kind, from, to) in [
                    (
                        knight,
                        PieceType::Knight,
                        Vec2::new(6.0, 0.0),
                        Vec2::new(5.0, 2.0),
                    ),
                    (
                        rook,
                        PieceType::Rook,
                        Vec2::new(7.0, 0.0),
                        Vec2::new(5.0, 0.0),
                    ),
                ] {
                    let sample = MotionStyle::GAMEPLAY.sample(elapsed, 0.4, from.distance(to));
                    moving.insert(
                        entity,
                        MovingPiece {
                            position: sample.position2(from, to),
                            lift: sample.lift,
                            piece: (kind, PieceColor::White),
                        },
                    );
                }
                let view = BoardView {
                    black_view: false,
                    is_human: false,
                    game_over: false,
                    piece_alpha: 255,
                    in_check: false,
                    check_color: PieceColor::Black,
                    last_move: None,
                    theme: Board2DTheme::default(),
                    capture_flashes: HashMap::new(),
                    pieces: HashMap::from([
                        ((5, 2), (PieceType::Knight, PieceColor::White, knight)),
                        ((5, 0), (PieceType::Rook, PieceColor::White, rook)),
                    ]),
                    moving,
                    selected: None,
                    legal_moves: Vec::new(),
                    is_selected: false,
                    textures: HashMap::from([
                        (
                            (PieceType::Knight, PieceColor::White),
                            egui::TextureId::User(101),
                        ),
                        (
                            (PieceType::Rook, PieceColor::White),
                            egui::TextureId::User(102),
                        ),
                    ]),
                };
                let ctx = egui::Context::default();
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        let geom = BoardGeom {
                            rect: egui::Rect::from_min_size(
                                egui::pos2(30.0, 30.0),
                                egui::vec2(400.0, 400.0),
                            ),
                            square: 50.0,
                            black_view: false,
                        };
                        let painter = ui.painter().clone();
                        let mut input = BoardInput::default();
                        for (file, rank) in [(5, 2), (5, 0)] {
                            draw_square(ui, &painter, geom, file, rank, &view, &extras, &mut input);
                        }
                        draw_moving_pieces(&painter, geom, &view, &extras);
                    },
                );
                for texture in [101, 102] {
                    let count = output.shapes.iter().filter(|shape| matches!(&shape.shape, egui::epaint::Shape::Mesh(mesh) if mesh.texture_id == egui::TextureId::User(texture))).count();
                    assert_eq!(
                        count, 1,
                        "piece {texture}, elapsed={elapsed}, dragged={dragged}"
                    );
                }
                output.drop_without_applying_deltas();
            }
        }
        Ok(())
    }

    #[test]
    fn resizing_and_flipping_reproject_the_same_sample_without_advancing_time() {
        let mut tx = MoveTransaction::new(
            Vec2::new(6.0, 0.0),
            Vec2::new(5.0, 2.0),
            crate::motion::GAMEPLAY_MOVE_SECONDS,
            MotionStyle::GAMEPLAY,
        );
        tx.advance(0.16);
        let before = tx;
        let small = BoardGeom {
            rect: egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(400.0, 400.0)),
            square: 50.0,
            black_view: false,
        };
        let large = BoardGeom {
            square: 100.0,
            ..small
        };
        let flipped = BoardGeom {
            black_view: true,
            ..small
        };
        let position = tx.position();
        let a = small.point_center(position) - small.rect.min;
        let b = large.point_center(position) - large.rect.min;
        let c = flipped.point_center(position) - flipped.rect.min;
        assert_eq!(b, a * 2.0);
        assert!((a.x + c.x - 400.0).abs() < 0.001);
        assert!((a.y + c.y - 400.0).abs() < 0.001);
        assert_eq!(tx, before);
        let world = crate::motion::board_to_world(position);
        assert_eq!(world.x, 7.0 - position.x);
        assert_eq!(world.z, position.y);
    }
}
