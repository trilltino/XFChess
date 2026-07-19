//! 2D game board rendering using egui
//!
//! Provides a lightweight 2D chess board interface using egui's immediate mode GUI.
//! This allows players to play chess with a traditional 2D view while maintaining
//! full compatibility with the existing game state and networking systems.

use crate::core::states::GameMode;
use crate::game::components::FadingCapture;
use crate::game::resources::{CurrentTurn, Players};
use crate::game::systems::camera::get_is_black_view;
use crate::game::systems::input::{
    can_move_color, clear_selection_state, is_human_turn, try_move_sequence, try_select_piece,
    InputSystemParams,
};
use crate::game::systems::shared::CapturedTarget;
use crate::game::view_mode::ViewMode;
use crate::rendering::pieces::{PieceColor, PieceType};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::HashMap;

/// Bundles extra 2D board resources to stay within the 16-param system limit.
#[derive(SystemParam)]
pub struct Board2DExtras<'w> {
    pub eval_bar: Res<'w, EvalBarState>,
    pub arrows: ResMut<'w, BoardArrows>,
    pub keyboard: Res<'w, ButtonInput<KeyCode>>,
    pub sounds: Option<Res<'w, crate::game::resources::GameSounds>>,
    pub settings: ResMut<'w, crate::core::GameSettings>,
    pub drag: ResMut<'w, DragState2D>,
    pub premove: ResMut<'w, PremoveState>,
    pub anim: ResMut<'w, PieceAnim2D>,
    pub time: Res<'w, Time>,
    pub promotion: Res<'w, crate::game::resources::PendingPromotion>,
    pub promotion_writer:
        bevy::prelude::MessageWriter<'w, crate::game::resources::PromotionSelected>,
    pub focus: ResMut<'w, BoardFocus>,
    pub cm_flash: Res<'w, CheckmateFlashState>,
    pub board_fade: Res<'w, BoardFadeState>,
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

/// Drag-to-move state for the 2D board.
#[derive(Resource, Default)]
pub struct DragState2D {
    pub dragging: bool,
    pub from: (u8, u8),
    pub cursor_pos: egui::Pos2,
    pub piece: Option<(PieceType, PieceColor)>,
}

/// Premove state: player can queue a move during opponent's turn.
#[derive(Resource, Default)]
pub struct PremoveState {
    pub from: Option<(u8, u8)>,
    pub to: Option<(u8, u8)>,
}

impl PremoveState {
    pub fn is_set(&self) -> bool {
        self.from.is_some() && self.to.is_some()
    }
    pub fn clear(&mut self) {
        self.from = None;
        self.to = None;
    }
}

/// 2D piece slide animation state.
#[derive(Resource, Default)]
pub struct PieceAnim2D {
    pub active: bool,
    pub from_px: egui::Pos2,
    pub to_px: egui::Pos2,
    pub elapsed: f32,
    pub piece: Option<(PieceType, PieceColor)>,
    pub from_sq: (u8, u8),
    pub to_sq: (u8, u8),
    /// Set to true once from_px/to_px have been computed for this animation.
    pub pixels_ready: bool,
}

impl PieceAnim2D {
    const DURATION: f32 = 0.15;
    pub fn t(&self) -> f32 {
        (self.elapsed / Self::DURATION).min(1.0)
    }
    pub fn lerped_pos(&self) -> egui::Pos2 {
        let t = self.t();
        egui::Pos2::new(
            self.from_px.x + (self.to_px.x - self.from_px.x) * t,
            self.from_px.y + (self.to_px.y - self.from_px.y) * t,
        )
    }
}

/// Triggers a 2D piece animation on `MoveMadeEvent`.
pub fn trigger_piece_anim_2d(
    mut events: bevy::prelude::MessageReader<crate::game::events::MoveMadeEvent>,
    mut anim: ResMut<PieceAnim2D>,
    view_mode: Res<ViewMode>,
) {
    for ev in events.read() {
        if *view_mode != ViewMode::Standard2D {
            continue;
        }
        anim.active = true;
        anim.elapsed = 0.0;
        anim.from_sq = ev.from;
        anim.to_sq = ev.to;
        anim.from_px = egui::Pos2::ZERO;
        anim.to_px = egui::Pos2::ZERO;
        anim.pixels_ready = false;
        anim.piece = None;
    }
}

/// Drives the amber flash on the mated king square for 2 s after checkmate.
#[derive(Resource, Default)]
pub struct CheckmateFlashState {
    pub active: bool,
    pub elapsed: f32,
    pub king_sq: (u8, u8),
}

impl CheckmateFlashState {
    const DURATION: f32 = 2.0;
    pub fn alpha(&self) -> u8 {
        let t = (self.elapsed / Self::DURATION).min(1.0);
        // Pulses: sin wave fading out
        let pulse = ((t * std::f32::consts::TAU * 3.0).sin() * 0.5 + 0.5) * (1.0 - t);
        (pulse * 200.0) as u8
    }
}

/// System: starts the checkmate flash when the game ends by checkmate.
pub fn trigger_checkmate_flash(
    game_over: Res<crate::game::resources::history::game_over::GameOverState>,
    mut flash: ResMut<CheckmateFlashState>,
    pieces: Query<&crate::rendering::pieces::Piece>,
) {
    if flash.active {
        return;
    }
    if !game_over.is_checkmate() {
        return;
    }
    // Find the king of the losing side (the side that was checkmated)
    use crate::rendering::pieces::{PieceColor, PieceType};
    // WhiteWon = Black was checkmated → Black king flashes
    let loser_color = match &*game_over {
        crate::game::resources::history::game_over::GameOverState::WhiteWon => PieceColor::Black,
        crate::game::resources::history::game_over::GameOverState::BlackWon => PieceColor::White,
        _ => return,
    };
    for piece in &pieces {
        if piece.piece_type == PieceType::King && piece.color == loser_color {
            flash.active = true;
            flash.elapsed = 0.0;
            flash.king_sq = (piece.x, piece.y);
            return;
        }
    }
}

/// System: ticks the checkmate flash timer.
pub fn tick_checkmate_flash(mut flash: ResMut<CheckmateFlashState>, time: Res<Time>) {
    if !flash.active {
        return;
    }
    flash.elapsed += time.delta_secs();
    if flash.elapsed >= CheckmateFlashState::DURATION {
        flash.active = false;
    }
}

/// Fades the 2D board to 40% opacity on resignation.
#[derive(Resource)]
pub struct BoardFadeState {
    /// Current opacity multiplier (1.0 = normal, 0.4 = resigned-out).
    pub alpha_mult: f32,
    pub target: f32,
}

impl Default for BoardFadeState {
    fn default() -> Self {
        Self {
            alpha_mult: 1.0,
            target: 1.0,
        }
    }
}

/// System: triggers board fade on resign; ticks lerp each frame.
pub fn board_fade_system(
    game_over: Res<crate::game::resources::history::game_over::GameOverState>,
    mut fade: ResMut<BoardFadeState>,
    time: Res<Time>,
) {
    if game_over.is_resignation() {
        fade.target = 0.4;
    }
    fade.alpha_mult += (fade.target - fade.alpha_mult) * (time.delta_secs() * 3.0).min(1.0);
}

/// Keyboard navigation cursor for the 2D board.
#[derive(Resource)]
pub struct BoardFocus {
    /// Whether keyboard nav is active (toggled by Tab).
    pub active: bool,
    /// Current cursor square (file, rank).
    pub cursor: (u8, u8),
}

impl Default for BoardFocus {
    fn default() -> Self {
        Self {
            active: false,
            cursor: (4, 1),
        }
    }
}

/// Right-click arrow annotations drawn over the board.
#[derive(Resource, Default)]
pub struct BoardArrows {
    /// Stored arrows: (from_file, from_rank, to_file, to_rank, color_kind)
    /// color_kind: 0=green, 1=orange (Shift), 2=blue (Alt)
    pub arrows: Vec<(u8, u8, u8, u8, u8)>,
    pub drag_from: Option<(u8, u8)>,
}

/// Per-ply centipawn scores for annotation chips in move history.
/// scores[i] = eval (white perspective) after ply i.
#[derive(Resource, Default)]
pub struct EvalHistory {
    pub scores: Vec<i16>,
    /// Cached game state after the last evaluated move — avoids replaying from move 1.
    cached_game: Option<nimzovich_engine::Game>,
}

/// Centipawn evaluation bar state (updated each move, drives the visual bar).
#[derive(Resource, Default)]
pub struct EvalBarState {
    /// Centipawn score from White's perspective. Positive = White better.
    pub score: i16,
    /// Whether the bar is visible (toggled from sidebar / settings).
    pub visible: bool,
}

impl EvalBarState {
    /// White fill fraction 0.0 (Black winning heavily) – 1.0 (White winning heavily).
    /// Clamped at ±500 cp → 100%.
    pub fn white_fraction(&self) -> f32 {
        let clamped = self.score.clamp(-500, 500) as f32;
        (clamped + 500.0) / 1000.0
    }
}

/// System that recomputes the eval score whenever MoveHistory changes.
/// Also builds per-ply eval history for move annotation chips.
pub fn update_eval_bar(
    history: Res<crate::game::resources::MoveHistory>,
    mut eval: ResMut<EvalBarState>,
    mut eval_history: ResMut<EvalHistory>,
    game_mode: Res<crate::core::states::GameMode>,
) {
    use crate::core::states::GameMode;
    if !history.is_changed() {
        return;
    }
    // Hide eval in competitive online (anti-cheat)
    if matches!(*game_mode, GameMode::MultiplayerCompetitive) {
        eval.score = 0;
        eval_history.scores.clear();
        return;
    }
    use crate::game::components::PieceType;
    use nimzovich_engine::{do_move_with_promo, evaluate_position, new_game};

    let moves = &history.moves;
    let cached_count = eval_history.scores.len();

    // If history shrank (e.g. new game started), reset cached state.
    if cached_count > moves.len() {
        eval_history.scores.clear();
        eval_history.cached_game = None;
    }

    // Nothing new to evaluate.
    if eval_history.scores.len() == moves.len() {
        return;
    }

    // Start from the cached game state and apply only the new moves.
    let start = eval_history.scores.len();
    let mut game = eval_history.cached_game.take().unwrap_or_else(new_game);
    for rec in &moves[start..] {
        let src = rec.from.1 as i8 * 8 + rec.from.0 as i8;
        let dst = rec.to.1 as i8 * 8 + rec.to.0 as i8;
        let is_promo = rec.piece_type == PieceType::Pawn && (rec.to.1 == 7 || rec.to.1 == 0);
        let promo: i8 = if is_promo { 5 } else { 0 };
        do_move_with_promo(&mut game, src, dst, true, promo);
        eval_history.scores.push(evaluate_position(&game));
    }
    eval_history.cached_game = Some(game);
    eval.score = eval_history.scores.last().copied().unwrap_or(0);
}

/// Color theme for the 2D board.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Board2DTheme {
    pub dark_sq: egui::Color32,
    pub light_sq: egui::Color32,
}

impl Default for Board2DTheme {
    fn default() -> Self {
        Self::classic()
    }
}

impl Board2DTheme {
    pub fn classic() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(181, 136, 99),
            light_sq: egui::Color32::from_rgb(240, 217, 181),
        }
    }
    pub fn green() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(118, 150, 86),
            light_sq: egui::Color32::from_rgb(238, 238, 210),
        }
    }
    pub fn blue() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(70, 130, 180),
            light_sq: egui::Color32::from_rgb(200, 220, 240),
        }
    }
    pub fn purple() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(100, 80, 150),
            light_sq: egui::Color32::from_rgb(220, 210, 240),
        }
    }
    pub fn dark() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(60, 60, 70),
            light_sq: egui::Color32::from_rgb(115, 115, 125),
        }
    }

    pub fn sq_color(&self, file: u8, rank: u8) -> egui::Color32 {
        if (file + rank) % 2 == 0 {
            self.dark_sq
        } else {
            self.light_sq
        }
    }

    /// A contrasting label color for corner labels inside this square.
    pub fn label_color(&self, file: u8, rank: u8) -> egui::Color32 {
        if (file + rank) % 2 == 0 {
            self.light_sq
        } else {
            self.dark_sq
        }
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
/// Sync Board2DTheme from GameSettings on startup / settings change.
pub fn sync_board_theme_from_settings(
    settings: Res<crate::core::GameSettings>,
    mut theme: ResMut<Board2DTheme>,
) {
    if !settings.is_changed() {
        return;
    }
    *theme = theme_from_index(settings.board_theme);
}

/// Convert a theme index (0–4) to a Board2DTheme.
pub fn theme_from_index(idx: u8) -> Board2DTheme {
    match idx {
        1 => Board2DTheme::green(),
        2 => Board2DTheme::blue(),
        3 => Board2DTheme::purple(),
        4 => Board2DTheme::dark(),
        _ => Board2DTheme::classic(),
    }
}

/// Convert a Board2DTheme to its closest index.
fn theme_to_index(t: &Board2DTheme) -> u8 {
    if t.dark_sq == Board2DTheme::green().dark_sq {
        return 1;
    }
    if t.dark_sq == Board2DTheme::blue().dark_sq {
        return 2;
    }
    if t.dark_sq == Board2DTheme::purple().dark_sq {
        return 3;
    }
    if t.dark_sq == Board2DTheme::dark().dark_sq {
        return 4;
    }
    0
}

pub fn render_2d_board(
    mut input_params: InputSystemParams,
    mut contexts: bevy_egui::EguiContexts,
    sprite_handles: Option<Res<crate::rendering::pieces::PieceSpriteHandles>>,
    _game_phase: Res<crate::game::resources::CurrentGamePhase>,
    #[cfg(feature = "solana")] _solana_profile: Option<
        Res<crate::multiplayer::solana::addon::SolanaProfile>,
    >,
    #[cfg(feature = "solana")] _competitive_match: Option<
        Res<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    >,
    view_mode: Res<ViewMode>,
    game_mode: Res<GameMode>,
    players: Res<Players>,
    current_turn: Res<CurrentTurn>,
    _hud_visibility: Res<crate::ui::game::game_ui::InGameHudVisibility>,
    fading_captures: Query<&FadingCapture>,
    mut theme: ResMut<Board2DTheme>,
    mut extras: Board2DExtras,
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

    // Pre-calculate texture IDs for piece sprites to avoid borrow conflicts with contexts.ctx_mut()
    let mut texture_map: HashMap<(PieceType, PieceColor), egui::TextureId> = HashMap::new();
    if let Some(handles) = &sprite_handles {
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
    let piece_alpha = (extras.board_fade.alpha_mult * 255.0).clamp(0.0, 255.0) as u8;
    let in_check = input_params.engine.is_check();
    let check_color = input_params.engine.current_turn;

    // Last move squares for highlight
    let last_move_squares: Option<((u8, u8), (u8, u8))> = input_params
        .move_history
        .moves
        .last()
        .map(|m| (m.from, m.to));

    // Collect theme value to avoid borrow in closure
    let current_theme = *theme;
    let mut pending_theme: Option<Board2DTheme> = None;

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
    let mut promo_chosen: Option<PieceType> = None;
    // Variables filled inside the closure for drag-to-move.
    let mut drag_started_at: Option<(u8, u8)> = None;
    let mut drag_released_at: Option<(u8, u8)> = None;
    let mut drag_cursor_update: Option<egui::Pos2> = None;
    // Variables filled inside the closure for premove.
    let mut premove_click: Option<(u8, u8)> = None;

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

                // Allocate board space; right-click-drag for arrow drawing.
                let board_id = egui::Id::new("board_main_rect");
                let (board_rect, board_resp) = ui.allocate_exact_size(
                    egui::Vec2::splat(board_size),
                    egui::Sense::click_and_drag(),
                );
                // Determine color kind from modifier keys
                let arrow_kind: u8 = if extras
                    .keyboard
                    .any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
                {
                    1
                } else if extras
                    .keyboard
                    .any_pressed([KeyCode::AltLeft, KeyCode::AltRight])
                {
                    2
                } else {
                    0
                };
                // Convert screen position to board square
                let pos_to_sq = |pos: egui::Pos2| -> Option<(u8, u8)> {
                    let rel = pos - board_rect.min;
                    if rel.x < 0.0 || rel.y < 0.0 || rel.x >= board_size || rel.y >= board_size {
                        return None;
                    }
                    let col = (rel.x / square_size) as u8;
                    let row = (rel.y / square_size) as u8;
                    let (file, rank) = if black_view {
                        (7 - col, row)
                    } else {
                        (col, 7 - row)
                    };
                    Some((file, rank))
                };
                // Right-click drag start
                if board_resp.drag_started_by(egui::PointerButton::Secondary) {
                    if let Some(pos) = board_resp.interact_pointer_pos() {
                        extras.arrows.drag_from = pos_to_sq(pos);
                    }
                }
                // Right-click drag release → store arrow
                if board_resp.drag_stopped_by(egui::PointerButton::Secondary) {
                    if let Some(from) = extras.arrows.drag_from.take() {
                        if let Some(pos) = board_resp.interact_pointer_pos() {
                            if let Some(to) = pos_to_sq(pos) {
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
                }
                // Right-click without drag — also clear
                if board_resp.secondary_clicked() && extras.arrows.drag_from.is_none() {
                    extras.arrows.arrows.clear();
                }
                // Left-click drag start → drag-to-move
                if board_resp.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(pos) = board_resp.interact_pointer_pos() {
                        drag_started_at = pos_to_sq(pos);
                    }
                }
                // Left-click drag in progress → update cursor
                if extras.drag.dragging {
                    if let Some(pos) = board_resp.interact_pointer_pos() {
                        drag_cursor_update = Some(pos);
                    }
                }
                // Left-click drag release → execute or cancel
                if board_resp.drag_stopped_by(egui::PointerButton::Primary) && extras.drag.dragging
                {
                    if let Some(pos) = board_resp.interact_pointer_pos() {
                        drag_released_at = pos_to_sq(pos);
                    } else {
                        drag_released_at = Some(extras.drag.from); // cancelled (released off board)
                    }
                }
                let _ = board_id;

                let painter = ui.painter_at(board_rect);

                for rank in 0..8u8 {
                    for file in 0..8u8 {
                        let offset = board_to_screen(file, rank, black_view, square_size);
                        let sq_rect = egui::Rect::from_min_size(
                            board_rect.min + offset,
                            egui::Vec2::splat(square_size),
                        );

                        // Base square color from active theme
                        painter.rect_filled(sq_rect, 0.0, current_theme.sq_color(file, rank));

                        // Last-move highlight (golden tint, below selection)
                        if let Some((from, to)) = last_move_squares {
                            if (file, rank) == from || (file, rank) == to {
                                painter.rect_filled(
                                    sq_rect,
                                    0.0,
                                    egui::Color32::from_rgba_unmultiplied(205, 210, 100, 90),
                                );
                            }
                        }

                        // Premove highlight (cyan tint)
                        let is_premove_sq = extras.premove.from == Some((file, rank))
                            || extras.premove.to == Some((file, rank));
                        if is_premove_sq {
                            painter.rect_filled(
                                sq_rect,
                                0.0,
                                egui::Color32::from_rgba_unmultiplied(0, 200, 220, 100),
                            );
                        }

                        // Check king tint (red glow on the king in check)
                        if in_check && !game_over {
                            if let Some((pt, pc, _)) = piece_map.get(&(file, rank)) {
                                if *pt == PieceType::King && *pc == check_color {
                                    painter.rect_filled(
                                        sq_rect,
                                        0.0,
                                        egui::Color32::from_rgba_unmultiplied(200, 30, 30, 130),
                                    );
                                }
                            }
                        }

                        // Checkmate amber flash on mated king square
                        if extras.cm_flash.active && (file, rank) == extras.cm_flash.king_sq {
                            let a = extras.cm_flash.alpha();
                            painter.rect_filled(
                                sq_rect,
                                0.0,
                                egui::Color32::from_rgba_unmultiplied(255, 160, 0, a),
                            );
                        }

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

                        // Skip piece draw if this square is the drag source or
                        // the animation destination (drawn separately below).
                        let skip_piece = (extras.drag.dragging && extras.drag.from == (file, rank))
                            || (extras.anim.active && extras.anim.to_sq == (file, rank));

                        if !extras.settings.blindfold && !skip_piece {
                            if let Some((pt, pc, _)) = piece_map.get(&(file, rank)) {
                                let mut piece_drawn = false;

                                // Try to draw sprite first
                                if let Some(id) = texture_map.get(&(*pt, *pc)) {
                                    painter.image(
                                        *id,
                                        sq_rect.shrink(square_size * 0.1),
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        egui::Color32::from_rgba_unmultiplied(
                                            255,
                                            255,
                                            255,
                                            piece_alpha,
                                        ),
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
                        } // end !blindfold && !skip_piece draw

                        // In-board corner coordinate labels (Lichess style)
                        let is_bottom_row = if black_view { rank == 7 } else { rank == 0 };
                        let is_left_col = if black_view { file == 7 } else { file == 0 };
                        let lc = current_theme.label_color(file, rank);
                        let font = egui::FontId::proportional(9.0);
                        if is_left_col {
                            painter.text(
                                sq_rect.min + egui::Vec2::new(2.0, 2.0),
                                egui::Align2::LEFT_TOP,
                                (rank + 1).to_string(),
                                font.clone(),
                                lc,
                            );
                        }
                        if is_bottom_row {
                            painter.text(
                                sq_rect.max - egui::Vec2::new(2.0, 2.0),
                                egui::Align2::RIGHT_BOTTOM,
                                ((b'a' + file) as char).to_string(),
                                font,
                                lc,
                            );
                        }

                        // Keyboard nav focus ring
                        if extras.focus.active && extras.focus.cursor == (file, rank) {
                            painter.rect_stroke(
                                sq_rect,
                                0.0,
                                egui::Stroke::new(2.5, egui::Color32::from_rgb(0, 210, 240)),
                                egui::StrokeKind::Outside,
                            );
                        }

                        // Per-square click detection.
                        let sq_id = egui::Id::new("board_sq").with(file as u32 * 8 + rank as u32);
                        let sq_resp = ui.interact(sq_rect, sq_id, egui::Sense::click());
                        if sq_resp.clicked() {
                            if is_human && !game_over {
                                clicked_square = Some((file, rank));
                            } else if !is_human && !game_over {
                                premove_click = Some((file, rank));
                            }
                        }
                    }
                }

                // ── Animated piece overlay ───────────────────────────────────
                if extras.anim.active && !extras.settings.blindfold {
                    // Compute pixel centers on the first render frame of this animation.
                    if !extras.anim.pixels_ready {
                        let from_off = board_to_screen(
                            extras.anim.from_sq.0,
                            extras.anim.from_sq.1,
                            black_view,
                            square_size,
                        );
                        let to_off = board_to_screen(
                            extras.anim.to_sq.0,
                            extras.anim.to_sq.1,
                            black_view,
                            square_size,
                        );
                        extras.anim.from_px =
                            board_rect.min + from_off + egui::Vec2::splat(square_size * 0.5);
                        extras.anim.to_px =
                            board_rect.min + to_off + egui::Vec2::splat(square_size * 0.5);
                        extras.anim.pixels_ready = true;
                        // Fill piece identity from piece_map (piece is now at to_sq in ECS).
                        if extras.anim.piece.is_none() {
                            if let Some(&(pt, pc, _)) = piece_map.get(&extras.anim.to_sq) {
                                extras.anim.piece = Some((pt, pc));
                            }
                        }
                    }
                    if let Some((pt, pc)) = extras.anim.piece {
                        let center = extras.anim.lerped_pos();
                        let half = square_size * 0.4;
                        let rect =
                            egui::Rect::from_center_size(center, egui::Vec2::splat(half * 2.0));
                        let mut drawn = false;
                        if let Some(id) = texture_map.get(&(pt, pc)) {
                            painter.image(
                                *id,
                                rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                            drawn = true;
                        }
                        if !drawn {
                            let symbol = piece_symbol(pt, pc);
                            let col = if pc == PieceColor::White {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_rgb(18, 18, 18)
                            };
                            painter.text(
                                center,
                                egui::Align2::CENTER_CENTER,
                                symbol,
                                egui::FontId::proportional(square_size * 0.72),
                                col,
                            );
                        }
                    }
                }

                // ── Dragged piece at cursor ──────────────────────────────────
                if extras.drag.dragging && !extras.settings.blindfold {
                    if let Some((pt, pc)) = extras.drag.piece {
                        let pos = extras.drag.cursor_pos;
                        let half = square_size * 0.45;
                        let rect = egui::Rect::from_center_size(pos, egui::Vec2::splat(half * 2.0));
                        let mut drawn = false;
                        if let Some(id) = texture_map.get(&(pt, pc)) {
                            painter.image(
                                *id,
                                rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                            drawn = true;
                        }
                        if !drawn {
                            let symbol = piece_symbol(pt, pc);
                            let col = if pc == PieceColor::White {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_rgb(18, 18, 18)
                            };
                            painter.text(
                                pos,
                                egui::Align2::CENTER_CENTER,
                                symbol,
                                egui::FontId::proportional(square_size * 0.72),
                                col,
                            );
                        }
                    }
                }

                // ── Inline promotion picker ──────────────────────────────────
                if extras.promotion.is_active() {
                    if let (Some((pf, pr)), Some(pcolor)) =
                        (extras.promotion.position, extras.promotion.color)
                    {
                        // Dim the rest of the board
                        painter.rect_filled(
                            board_rect,
                            0.0,
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
                        );
                        // Four piece options: Q, R, B, N — arranged vertically toward the board center
                        let pieces_order = [
                            PieceType::Queen,
                            PieceType::Rook,
                            PieceType::Bishop,
                            PieceType::Knight,
                        ];
                        let prom_rank_screen_top = pr == 7; // white promotes on rank 7 → column goes downward
                        for (idx, &pt) in pieces_order.iter().enumerate() {
                            let btn_rank = if prom_rank_screen_top {
                                pr.saturating_sub(idx as u8)
                            } else {
                                pr.saturating_add(idx as u8).min(7)
                            };
                            let offset = board_to_screen(pf, btn_rank, black_view, square_size);
                            let btn_rect = egui::Rect::from_min_size(
                                board_rect.min + offset,
                                egui::Vec2::splat(square_size),
                            );
                            // Background
                            let bg = if idx % 2 == 0 {
                                egui::Color32::from_rgb(240, 217, 181)
                            } else {
                                egui::Color32::from_rgb(181, 136, 99)
                            };
                            painter.rect_filled(btn_rect, 4.0, bg);
                            // Draw piece
                            let symbol = piece_symbol(pt, pcolor);
                            let font_size = square_size * 0.72;
                            let piece_col = if pcolor == PieceColor::White {
                                egui::Color32::from_rgb(18, 18, 18)
                            } else {
                                egui::Color32::WHITE
                            };
                            // Shadow
                            let shadow_col = if pcolor == PieceColor::White {
                                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 60)
                            };
                            painter.text(
                                btn_rect.center() + egui::Vec2::new(1.5, 1.5),
                                egui::Align2::CENTER_CENTER,
                                symbol,
                                egui::FontId::proportional(font_size),
                                shadow_col,
                            );
                            painter.text(
                                btn_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                symbol,
                                egui::FontId::proportional(font_size),
                                piece_col,
                            );
                            // Click detection
                            let btn_id = egui::Id::new("promo_btn").with(idx as u32);
                            let btn_resp = ui.interact(btn_rect, btn_id, egui::Sense::click());
                            if btn_resp.clicked() {
                                promo_chosen = Some(pt); // captured by outer scope
                            }
                            // Hover highlight
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
                }

                // ── Arrow overlays ───────────────────────────────────────────
                for &(ff, fr, tf, tr, kind) in &extras.arrows.arrows {
                    let from_offset = board_to_screen(ff, fr, black_view, square_size);
                    let to_offset = board_to_screen(tf, tr, black_view, square_size);
                    let from_c =
                        board_rect.min + from_offset + egui::Vec2::splat(square_size * 0.5);
                    let to_c = board_rect.min + to_offset + egui::Vec2::splat(square_size * 0.5);
                    let col = match kind {
                        1 => egui::Color32::from_rgba_unmultiplied(255, 140, 0, 180),
                        2 => egui::Color32::from_rgba_unmultiplied(80, 160, 255, 180),
                        _ => egui::Color32::from_rgba_unmultiplied(20, 200, 60, 180),
                    };
                    painter.arrow(
                        from_c,
                        to_c - from_c,
                        egui::Stroke::new(square_size * 0.12, col),
                    );
                }
                // Draw in-progress drag arrow
                if let Some(from) = extras.arrows.drag_from {
                    if let Some(cursor) = board_resp.interact_pointer_pos() {
                        let from_off = board_to_screen(from.0, from.1, black_view, square_size);
                        let from_c =
                            board_rect.min + from_off + egui::Vec2::splat(square_size * 0.5);
                        let col = match arrow_kind {
                            1 => egui::Color32::from_rgba_unmultiplied(255, 140, 0, 120),
                            2 => egui::Color32::from_rgba_unmultiplied(80, 160, 255, 120),
                            _ => egui::Color32::from_rgba_unmultiplied(20, 200, 60, 120),
                        };
                        painter.arrow(
                            from_c,
                            cursor - from_c,
                            egui::Stroke::new(square_size * 0.10, col),
                        );
                    }
                }

                // ── Keyboard navigation ──────────────────────────────────────
                if extras.keyboard.just_pressed(KeyCode::Tab) {
                    extras.focus.active = !extras.focus.active;
                }
                if extras.focus.active && !game_over {
                    let (mut f, mut r) = extras.focus.cursor;
                    if extras.keyboard.just_pressed(KeyCode::ArrowRight) {
                        if black_view {
                            f = f.saturating_sub(1);
                        } else {
                            f = (f + 1).min(7);
                        }
                    } else if extras.keyboard.just_pressed(KeyCode::ArrowLeft) {
                        if black_view {
                            f = (f + 1).min(7);
                        } else {
                            f = f.saturating_sub(1);
                        }
                    } else if extras.keyboard.just_pressed(KeyCode::ArrowUp) {
                        r = (r + 1).min(7);
                    } else if extras.keyboard.just_pressed(KeyCode::ArrowDown) {
                        r = r.saturating_sub(1);
                    }
                    extras.focus.cursor = (f, r);
                    if extras.keyboard.just_pressed(KeyCode::Enter)
                        || extras.keyboard.just_pressed(KeyCode::Space)
                    {
                        let sq = extras.focus.cursor;
                        if is_human {
                            clicked_square = Some(sq);
                        } else {
                            premove_click = Some(sq);
                        }
                    }
                }

                // ── Evaluation bar (right of board) ──────────────────────────
                if extras.eval_bar.visible {
                    let bar_w = 14.0_f32;
                    let bar_x = board_rect.max.x + 8.0;
                    let bar_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(bar_x, board_rect.min.y),
                        egui::Vec2::new(bar_w, board_size),
                    );
                    // Background
                    painter.rect_filled(bar_rect, 3.0, egui::Color32::from_rgb(40, 40, 40));
                    // White fill (bottom portion)
                    let frac = extras.eval_bar.white_fraction();
                    let white_h = board_size * frac;
                    let white_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(bar_x, board_rect.max.y - white_h),
                        egui::Vec2::new(bar_w, white_h),
                    );
                    painter.rect_filled(white_rect, 3.0, egui::Color32::from_rgb(230, 230, 230));
                    // Score label
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

    // Theme picker — small dot row anchored bottom-center.
    // Runs outside CentralPanel so we can mutably borrow `theme` here.
    egui::Window::new("board_theme_picker")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -10.0])
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 25, 180))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(8, 5)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                ui.label(
                    egui::RichText::new("Board:")
                        .size(10.0)
                        .color(egui::Color32::from_gray(140)),
                );
                for (t, name) in [
                    (Board2DTheme::classic(), "Classic"),
                    (Board2DTheme::green(), "Green"),
                    (Board2DTheme::blue(), "Blue"),
                    (Board2DTheme::purple(), "Purple"),
                    (Board2DTheme::dark(), "Dark"),
                ] {
                    let active = current_theme.dark_sq == t.dark_sq;
                    let size = egui::Vec2::splat(if active { 16.0 } else { 12.0 });
                    let stroke = if active {
                        egui::Stroke::new(2.0, egui::Color32::WHITE)
                    } else {
                        egui::Stroke::NONE
                    };
                    let r = ui.add(
                        egui::Button::new("")
                            .fill(t.dark_sq)
                            .stroke(stroke)
                            .min_size(size)
                            .corner_radius(3.0),
                    );
                    if r.on_hover_text(name).clicked() {
                        pending_theme = Some(t);
                    }
                    let _ = name;
                }
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                ui.label(
                    egui::RichText::new("Pieces:")
                        .size(10.0)
                        .color(egui::Color32::from_gray(140)),
                );
                for (idx, label) in [(0u8, "CB"), (1, "α"), (2, "M")] {
                    let active = extras.settings.piece_set == idx;
                    let stroke = if active {
                        egui::Stroke::new(1.5, egui::Color32::WHITE)
                    } else {
                        egui::Stroke::NONE
                    };
                    let btn_col = if active {
                        egui::Color32::from_rgb(80, 120, 180)
                    } else {
                        egui::Color32::from_gray(55)
                    };
                    let r = ui.add(
                        egui::Button::new(egui::RichText::new(label).size(9.0))
                            .fill(btn_col)
                            .stroke(stroke)
                            .min_size(egui::Vec2::new(20.0, 14.0))
                            .corner_radius(3.0),
                    );
                    let hover = match idx {
                        1 => "Alpha",
                        2 => "Merida",
                        _ => "CBurnett",
                    };
                    if r.on_hover_text(hover).clicked() {
                        extras.settings.piece_set = idx;
                    }
                }
                ui.add_space(6.0);
                // Eval bar toggle
                let eval_active = extras.eval_bar.visible;
                let eval_col = if eval_active {
                    egui::Color32::from_rgb(60, 160, 80)
                } else {
                    egui::Color32::from_gray(55)
                };
                let eval_stroke = if eval_active {
                    egui::Stroke::new(1.5, egui::Color32::WHITE)
                } else {
                    egui::Stroke::NONE
                };
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("E").size(9.0))
                            .fill(eval_col)
                            .stroke(eval_stroke)
                            .min_size(egui::Vec2::new(16.0, 14.0))
                            .corner_radius(3.0),
                    )
                    .on_hover_text("Eval bar (offline only)")
                    .clicked()
                {
                    // toggle via DerefMut — eval_bar is Res so we need a separate path
                    extras.settings.show_eval_bar = !extras.settings.show_eval_bar;
                }
                // Blindfold toggle
                let bf_col = if extras.settings.blindfold {
                    egui::Color32::from_rgb(160, 60, 60)
                } else {
                    egui::Color32::from_gray(55)
                };
                let bf_stroke = if extras.settings.blindfold {
                    egui::Stroke::new(1.5, egui::Color32::WHITE)
                } else {
                    egui::Stroke::NONE
                };
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("B").size(9.0))
                            .fill(bf_col)
                            .stroke(bf_stroke)
                            .min_size(egui::Vec2::new(16.0, 14.0))
                            .corner_radius(3.0),
                    )
                    .on_hover_text("Blindfold (Ctrl+B)")
                    .clicked()
                {
                    extras.settings.blindfold = !extras.settings.blindfold;
                }
            });
        });

    if let Some(t) = pending_theme {
        extras.settings.board_theme = theme_to_index(&t);
        *theme = t;
    }

    // ── Promotion choice ─────────────────────────────────────────────────
    if let (Some(pt), Some(entity), Some(position)) = (
        promo_chosen,
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

    // Apply cursor update for dragged piece.
    if let Some(pos) = drag_cursor_update {
        extras.drag.cursor_pos = pos;
    }

    if game_over {
        return;
    }

    // ── Drag-to-move: start ──────────────────────────────────────────────
    if let Some(sq) = drag_started_at {
        let piece_at = {
            let q = input_params.pieces.p1();
            q.iter()
                .find(|(_, p, _, _)| p.x == sq.0 && p.y == sq.1)
                .map(|(e, p, _, _)| (e, *p))
        };
        if let Some((entity, piece)) = piece_at {
            if is_human && can_move_color(&input_params, piece.color) {
                extras.drag.dragging = true;
                extras.drag.from = sq;
                extras.drag.piece = Some((piece.piece_type, piece.color));
                try_select_piece(&mut input_params, entity, piece, true);
            }
        }
    }

    // ── Drag-to-move: release ────────────────────────────────────────────
    if let Some(to_sq) = drag_released_at {
        if extras.drag.dragging {
            extras.drag.dragging = false;
            let from_sq = extras.drag.from;
            extras.drag.piece = None;
            if to_sq != from_sq && legal_moves.contains(&to_sq) {
                let capture_info = {
                    let q = input_params.pieces.p1();
                    q.iter()
                        .find(|(_, p, _, _)| p.x == to_sq.0 && p.y == to_sq.1)
                        .map(|(e, p, _, _)| CapturedTarget {
                            entity: e,
                            piece_type: p.piece_type,
                            color: p.color,
                        })
                };
                try_move_sequence(&mut input_params, to_sq, capture_info, "2d_drag");
            } else {
                clear_selection_state(
                    &mut input_params.commands,
                    &mut input_params.selection,
                    &input_params.selected_pieces,
                );
            }
        }
    }

    // ── Premove: queue clicks during opponent's turn ─────────────────────
    if !is_human {
        if let Some(sq) = premove_click {
            if extras.premove.from.is_none() {
                // The local player's color is the opponent of whoever is currently moving.
                let local_color = match current_turn.color {
                    PieceColor::White => PieceColor::Black,
                    PieceColor::Black => PieceColor::White,
                };
                let has_own_piece = {
                    let q = input_params.pieces.p1();
                    q.iter()
                        .any(|(_, p, _, _)| p.x == sq.0 && p.y == sq.1 && p.color == local_color)
                };
                if has_own_piece {
                    extras.premove.from = Some(sq);
                }
            } else if extras.premove.to.is_none() {
                if Some(sq) != extras.premove.from {
                    extras.premove.to = Some(sq);
                } else {
                    extras.premove.clear(); // clicked same square: cancel
                }
            } else {
                extras.premove.clear(); // third click: cancel and restart
            }
        }
        return;
    }

    // ── Execute queued premove when it becomes our turn ──────────────────
    if is_human && extras.premove.is_set() {
        let from = extras.premove.from.unwrap();
        let to = extras.premove.to.unwrap();
        extras.premove.clear();
        // Select the from-piece and execute the premove move.
        let piece_at = {
            let q = input_params.pieces.p1();
            q.iter()
                .find(|(_, p, _, _)| p.x == from.0 && p.y == from.1)
                .map(|(e, p, _, _)| (e, *p))
        };
        if let Some((entity, piece)) = piece_at {
            if can_move_color(&input_params, piece.color) {
                try_select_piece(&mut input_params, entity, piece, true);
                let updated_legal_moves = input_params.selection.possible_moves.clone();
                if updated_legal_moves.contains(&to) {
                    let capture_info = {
                        let q = input_params.pieces.p1();
                        q.iter()
                            .find(|(_, p, _, _)| p.x == to.0 && p.y == to.1)
                            .map(|(e, p, _, _)| CapturedTarget {
                                entity: e,
                                piece_type: p.piece_type,
                                color: p.color,
                            })
                    };
                    try_move_sequence(&mut input_params, to, capture_info, "2d_premove");
                    return; // premove fired — don't process normal click
                }
            }
        }
    }

    // ── Normal click handling ────────────────────────────────────────────
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
                _ => {
                    if let Some(ref s) = extras.sounds {
                        if !extras.settings.muted {
                            input_params
                                .commands
                                .spawn(bevy::audio::AudioPlayer::new(s.illegal.clone()));
                        }
                    }
                    clear_selection_state(
                        &mut input_params.commands,
                        &mut input_params.selection,
                        &input_params.selected_pieces,
                    );
                }
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

/// Sync `GameSettings::show_eval_bar` → `EvalBarState::visible` each frame settings change.
pub fn sync_eval_bar_visibility(
    settings: Res<crate::core::resources::GameSettings>,
    mut eval: ResMut<EvalBarState>,
) {
    if settings.is_changed() {
        eval.visible = settings.show_eval_bar;
    }
}
