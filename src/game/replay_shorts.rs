//! Shorts creation features for PGN replay:
//!   - Annotation overlays (arrows + square highlights) in 2D (egui) and 3D (Bevy meshes)
//!   - Puzzle mode: hide the answer move, show "Can you find it?" UI
//!   - Screenshot export via Bevy's render pipeline

use crate::core::GameMode;
use crate::game::replay::{ParsedPgnGameResource, PgnReplayState};
use crate::game::view_mode::{PlayerViewPreferences, ViewMode};
use crate::multiplayer::traits::{MessageReader, MessageWriter};
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

// ─────────────────────────────────────────────────────────────────────────────
// Resources & Events
// ─────────────────────────────────────────────────────────────────────────────

/// Arrow and square-highlight annotations drawn over the board in replay mode.
#[derive(Resource, Default)]
pub struct ReplayAnnotations {
    /// (from_file, from_rank, to_file, to_rank, color_kind)
    /// color_kind: 0 = green, 1 = orange (Shift held), 2 = blue (Alt held)
    pub arrows: Vec<(u8, u8, u8, u8, u8)>,
    /// (file, rank, color_kind)
    pub highlights: Vec<(u8, u8, u8)>,
    /// Right-click drag origin for the in-progress arrow preview
    pub drag_from: Option<(u8, u8)>,
    /// Set whenever arrows/highlights change so 3D meshes rebuild exactly once
    pub dirty: bool,
}

/// Puzzle mode state, separate from ParsedPgnGameResource so it can be toggled at runtime.
#[derive(Resource, Default)]
pub struct PuzzleOverlay {
    pub enabled: bool,
    pub revealed: bool,
    /// Ply at which the puzzle starts — the move after this ply is the answer
    pub puzzle_ply: usize,
    /// FEN string typed by the user for custom position entry
    pub fen_input: String,
    pub show_fen_input: bool,
}

/// Fired (by the UI button) to take a screenshot this frame.
#[derive(Message, Default)]
pub struct ScreenshotRequested;

/// Marker on Bevy mesh entities spawned as 3D arrow/highlight annotations.
#[derive(Component)]
pub struct ReplayAnnotation3D;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn sq_center_3d(file: u8, rank: u8) -> Vec3 {
    Vec3::new(file as f32, 0.20, rank as f32)
}

fn kind_color_3d(kind: u8) -> Color {
    match kind {
        1 => Color::srgba(1.0, 0.55, 0.0, 0.75),
        2 => Color::srgba(0.31, 0.63, 1.0, 0.75),
        _ => Color::srgba(0.08, 0.78, 0.24, 0.75),
    }
}

fn kind_color_egui(kind: u8) -> egui::Color32 {
    match kind {
        1 => egui::Color32::from_rgba_unmultiplied(255, 140, 0, 180),
        2 => egui::Color32::from_rgba_unmultiplied(80, 160, 255, 180),
        _ => egui::Color32::from_rgba_unmultiplied(20, 200, 60, 180),
    }
}

fn kind_highlight_egui(kind: u8) -> egui::Color32 {
    match kind {
        1 => egui::Color32::from_rgba_unmultiplied(255, 140, 0, 120),
        2 => egui::Color32::from_rgba_unmultiplied(80, 160, 255, 120),
        _ => egui::Color32::from_rgba_unmultiplied(20, 200, 60, 120),
    }
}

/// Board-coordinates → egui offset within the board widget.
fn b2s(file: u8, rank: u8, sq: f32) -> egui::Vec2 {
    // White perspective: a-file left, rank-1 bottom
    egui::Vec2::new(file as f32 * sq, (7 - rank) as f32 * sq)
}

/// Screen position → board (file, rank), returns None if outside the board.
fn s2b(pos: egui::Pos2, board_min: egui::Pos2, sq: f32) -> Option<(u8, u8)> {
    let rel = pos - board_min;
    if rel.x < 0.0 || rel.y < 0.0 || rel.x >= sq * 8.0 || rel.y >= sq * 8.0 {
        return None;
    }
    let file = (rel.x / sq) as u8;
    let rank = 7 - (rel.y / sq) as u8;
    Some((file, rank))
}

fn piece_sym(piece_type: PieceType, color: PieceColor) -> &'static str {
    match (piece_type, color) {
        (PieceType::King,   PieceColor::White) => "♔",
        (PieceType::Queen,  PieceColor::White) => "♕",
        (PieceType::Rook,   PieceColor::White) => "♖",
        (PieceType::Bishop, PieceColor::White) => "♗",
        (PieceType::Knight, PieceColor::White) => "♘",
        (PieceType::Pawn,   PieceColor::White) => "♙",
        (PieceType::King,   PieceColor::Black) => "♚",
        (PieceType::Queen,  PieceColor::Black) => "♛",
        (PieceType::Rook,   PieceColor::Black) => "♜",
        (PieceType::Bishop, PieceColor::Black) => "♝",
        (PieceType::Knight, PieceColor::Black) => "♞",
        (PieceType::Pawn,   PieceColor::Black) => "♟",
        _ => "?",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// System: 2D board + annotation overlay (egui, runs in PgnReplay + 2D mode)
// ─────────────────────────────────────────────────────────────────────────────

/// Renders the full 2D board with piece symbols and annotation overlays using egui.
/// Replaces the regular `render_2d_board` (which is disabled in PgnReplay mode).
pub fn replay_2d_annotation_system(
    mut contexts: EguiContexts,
    mut annotations: ResMut<ReplayAnnotations>,
    game_mode: Res<GameMode>,
    view_prefs: Res<PlayerViewPreferences>,
    pieces: Query<&Piece>,
    keyboard: Res<ButtonInput<KeyCode>>,
    puzzle: Res<PuzzleOverlay>,
    replay: Res<PgnReplayState>,
    pgn: Option<Res<ParsedPgnGameResource>>,
) {
    if *game_mode != GameMode::PgnReplay {
        return;
    }
    if view_prefs.local_view != ViewMode::Standard2D {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: egui::Color32::from_rgb(20, 20, 20),
            ..Default::default()
        })
        .show(ctx, |ui| {
            // Board is 8×8 squares. Leave 260px on the right for the move list panel.
            let avail = ui.available_size();
            let right_panel_w = 260.0;
            let usable_w = (avail.x - right_panel_w).max(100.0);
            let board_size = usable_w.min(avail.y).floor();
            let sq = board_size / 8.0;

            let x_off = ((usable_w - board_size) / 2.0).max(0.0);
            let y_off = ((avail.y - board_size) / 2.0).max(0.0);

            ui.add_space(y_off);
            ui.horizontal(|ui| {
                ui.add_space(x_off);

                let (board_rect, board_resp) = ui.allocate_exact_size(
                    egui::Vec2::splat(board_size),
                    egui::Sense::click_and_drag(),
                );

                let kind: u8 =
                    if keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) { 1 }
                    else if keyboard.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]) { 2 }
                    else { 0 };

                // Right-click drag → draw arrow; click on same square → highlight toggle
                if board_resp.drag_started_by(egui::PointerButton::Secondary) {
                    if let Some(pos) = board_resp.interact_pointer_pos() {
                        annotations.drag_from = s2b(pos, board_rect.min, sq);
                    }
                }
                if board_resp.drag_stopped_by(egui::PointerButton::Secondary) {
                    if let Some(from) = annotations.drag_from.take() {
                        if let Some(pos) = board_resp.interact_pointer_pos() {
                            if let Some(to) = s2b(pos, board_rect.min, sq) {
                                if to != from {
                                    annotations.arrows.push((from.0, from.1, to.0, to.1, kind));
                                } else {
                                    let idx = annotations.highlights.iter().position(|&(f, r, _)| f == from.0 && r == from.1);
                                    if let Some(i) = idx {
                                        annotations.highlights.remove(i);
                                    } else {
                                        annotations.highlights.push((from.0, from.1, kind));
                                    }
                                }
                                annotations.dirty = true;
                            }
                        }
                    }
                }
                // Right-click with no drag → clear all
                if board_resp.secondary_clicked() && annotations.drag_from.is_none() {
                    annotations.arrows.clear();
                    annotations.highlights.clear();
                    annotations.dirty = true;
                }

                let painter = ui.painter_at(board_rect);

                // ── Board squares ──
                for rank in 0..8u8 {
                    for file in 0..8u8 {
                        let off = b2s(file, rank, sq);
                        let rect = egui::Rect::from_min_size(board_rect.min + off, egui::Vec2::splat(sq));
                        let light = (file + rank) % 2 == 1;
                        painter.rect_filled(
                            rect,
                            0.0,
                            if light { egui::Color32::from_rgb(240, 217, 181) }
                            else     { egui::Color32::from_rgb(181, 136, 99) },
                        );
                    }
                }

                // ── Last-move highlight ──
                if let Some(ref pgn_res) = pgn {
                    if replay.current_ply > 0 {
                        // Highlight is shown via the engine state — we just tint lightly
                        // (actual from/to squares would require parsing the last SAN)
                    }
                }

                // ── Square highlights ──
                for &(hf, hr, hk) in &annotations.highlights {
                    let off = b2s(hf, hr, sq);
                    let rect = egui::Rect::from_min_size(board_rect.min + off, egui::Vec2::splat(sq));
                    painter.rect_filled(rect, 0.0, kind_highlight_egui(hk));
                }

                // ── Coordinate labels ──
                for i in 0..8u8 {
                    let file_char = (b'a' + i) as char;
                    let rank_char = (b'1' + i) as char;
                    let label_col = egui::Color32::from_rgba_unmultiplied(60, 40, 20, 160);
                    let fs = egui::FontId::proportional((sq * 0.18).max(9.0));
                    // File label along bottom edge
                    let fx = board_rect.min.x + b2s(i, 0, sq).x + sq * 0.5;
                    painter.text(egui::Pos2::new(fx, board_rect.max.y - 9.0), egui::Align2::CENTER_BOTTOM, file_char.to_string(), fs.clone(), label_col);
                    // Rank label along left edge
                    let ry = board_rect.min.y + b2s(0, i, sq).y + sq * 0.5;
                    painter.text(egui::Pos2::new(board_rect.min.x + 2.0, ry), egui::Align2::LEFT_CENTER, rank_char.to_string(), fs.clone(), label_col);
                }

                // ── Pieces ──
                for piece in pieces.iter() {
                    let off = b2s(piece.x, piece.y, sq);
                    let center = board_rect.min + off + egui::Vec2::splat(sq * 0.5);
                    let sym = piece_sym(piece.piece_type, piece.color);
                    let font_size = (sq * 0.70).max(12.0);
                    // Drop shadow
                    let shadow = if piece.color == PieceColor::White {
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40)
                    };
                    painter.text(center + egui::Vec2::new(1.5, 2.0), egui::Align2::CENTER_CENTER, sym, egui::FontId::proportional(font_size), shadow);
                    let col = if piece.color == PieceColor::White { egui::Color32::WHITE } else { egui::Color32::from_rgb(18, 18, 18) };
                    painter.text(center, egui::Align2::CENTER_CENTER, sym, egui::FontId::proportional(font_size), col);
                }

                // ── Stored arrows ──
                for &(ff, fr, tf, tr, ak) in &annotations.arrows {
                    let fc = board_rect.min + b2s(ff, fr, sq) + egui::Vec2::splat(sq * 0.5);
                    let tc = board_rect.min + b2s(tf, tr, sq) + egui::Vec2::splat(sq * 0.5);
                    painter.arrow(fc, tc - fc, egui::Stroke::new(sq * 0.12, kind_color_egui(ak)));
                }

                // ── In-progress drag arrow ──
                if let Some(from) = annotations.drag_from {
                    if let Some(cursor) = board_resp.interact_pointer_pos() {
                        let fc = board_rect.min + b2s(from.0, from.1, sq) + egui::Vec2::splat(sq * 0.5);
                        let c = kind_color_egui(kind);
                        let preview_col = egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 120);
                        painter.arrow(fc, cursor - fc, egui::Stroke::new(sq * 0.10, preview_col));
                    }
                }

                // ── Puzzle "Can you find it?" overlay ──
                if puzzle.enabled && !puzzle.revealed {
                    let overlay_rect = egui::Rect::from_min_size(
                        board_rect.center() - egui::Vec2::new(120.0, 30.0),
                        egui::Vec2::new(240.0, 60.0),
                    );
                    painter.rect_filled(overlay_rect, 8.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180));
                    painter.text(
                        overlay_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "🤔 Can you find the move?",
                        egui::FontId::proportional(18.0),
                        egui::Color32::from_rgb(255, 220, 60),
                    );
                }

                // ── Annotation legend hint ──
                painter.text(
                    egui::Pos2::new(board_rect.max.x - 4.0, board_rect.max.y - 4.0),
                    egui::Align2::RIGHT_BOTTOM,
                    "Right-click drag = arrow • Shift/Alt = color • Right-click = clear",
                    egui::FontId::proportional(9.0),
                    egui::Color32::from_rgba_unmultiplied(200, 200, 200, 80),
                );
            });
        });
}

// ─────────────────────────────────────────────────────────────────────────────
// System: 3D annotation meshes (Bevy entities, runs in PgnReplay + 3D mode)
// ─────────────────────────────────────────────────────────────────────────────

/// Rebuilds 3D arrow and square-highlight mesh entities whenever annotations change.
pub fn replay_3d_annotations_system(
    mut commands: Commands,
    mut annotations: ResMut<ReplayAnnotations>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<ReplayAnnotation3D>>,
    view_prefs: Res<PlayerViewPreferences>,
    game_mode: Res<GameMode>,
) {
    if *game_mode != GameMode::PgnReplay {
        return;
    }
    if view_prefs.local_view != ViewMode::Standard3D {
        // If the view switched to 2D, ensure 3D annotations are despawned
        if annotations.dirty {
            for e in existing.iter() {
                commands.entity(e).despawn();
            }
            annotations.dirty = false;
        }
        return;
    }
    if !annotations.dirty {
        return;
    }
    annotations.dirty = false;

    // Despawn previous annotation entities
    for e in existing.iter() {
        commands.entity(e).despawn();
    }

    // ── Square highlights (flat disc at Y=0.07, just above the board surface) ──
    let hl_mesh = meshes.add(Circle::new(0.42));
    for &(hf, hr, hk) in &annotations.highlights {
        let col = kind_color_3d(hk);
        let mat = materials.add(StandardMaterial {
            base_color: col,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            ..default()
        });
        commands.spawn((
            Mesh3d(hl_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_xyz(hf as f32, 0.07, hr as f32)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ReplayAnnotation3D,
            bevy::picking::Pickable::IGNORE,
        ));
    }

    // ── Arrows: cylinder shaft + cone head ──
    for &(ff, fr, tf, tr, ak) in &annotations.arrows {
        let from_pos = sq_center_3d(ff, fr);
        let to_pos   = sq_center_3d(tf, tr);
        let len = from_pos.distance(to_pos);
        if len < 0.1 { continue; }

        let dir = (to_pos - from_pos).normalize();
        let col = kind_color_3d(ak);
        let mat = materials.add(StandardMaterial {
            base_color: col,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });

        // Rotate from Bevy's default Y-up axis to the arrow direction (in XZ plane)
        let rotation = Quat::from_rotation_arc(Vec3::Y, dir);

        // Cone head: 0.30 units long at the destination end
        let head_len: f32 = 0.30;
        let shaft_len = (len - head_len).max(0.05);

        // Cylinder shaft
        let shaft_mesh = meshes.add(Cylinder {
            radius: 0.07,
            half_height: shaft_len * 0.5,
        });
        let shaft_mid = from_pos + dir * (shaft_len * 0.5);
        commands.spawn((
            Mesh3d(shaft_mesh),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(shaft_mid).with_rotation(rotation),
            ReplayAnnotation3D,
            bevy::picking::Pickable::IGNORE,
        ));

        // Cone head
        let cone_mesh = meshes.add(Cone {
            radius: 0.18,
            height: head_len,
        });
        let cone_center = from_pos + dir * (shaft_len + head_len * 0.5);
        commands.spawn((
            Mesh3d(cone_mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(cone_center).with_rotation(rotation),
            ReplayAnnotation3D,
            bevy::picking::Pickable::IGNORE,
        ));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// System: Screenshot
// ─────────────────────────────────────────────────────────────────────────────

/// When a ScreenshotRequested event is received, saves a PNG to the user's Pictures folder.
pub fn replay_screenshot_system(
    mut events: MessageReader<ScreenshotRequested>,
    mut commands: Commands,
) {
    let mut fired = false;
    for _ in events.read() { fired = true; }
    if !fired { return; }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let pictures_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(|home| std::path::PathBuf::from(home).join("Pictures"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let path = pictures_dir.join(format!("xfchess_short_{}.png", timestamp));

    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));

    info!("[SHORTS] Screenshot saved to {}", path.display());
}

// ─────────────────────────────────────────────────────────────────────────────
// System: Clear annotations when ply changes
// ─────────────────────────────────────────────────────────────────────────────

/// Clears annotation arrows/highlights whenever the replay moves to a new ply.
/// Highlights persist on the same ply; arrows clear on any navigation.
pub fn clear_annotations_on_ply_change(
    replay: Res<PgnReplayState>,
    mut annotations: ResMut<ReplayAnnotations>,
) {
    if replay.is_changed() && replay.position_dirty {
        annotations.arrows.clear();
        annotations.highlights.clear();
        annotations.dirty = true;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cinematic resources + events
// ─────────────────────────────────────────────────────────────────────────────

/// Full-screen flash effect driven by cinematic events.
#[derive(Resource, Default)]
pub struct CinematicEffect {
    pub flash_color: [f32; 3], // RGB 0..1
    pub flash_alpha: f32,
    pub flash_decay: f32, // alpha reduction per second
}

/// Drives the quality-badge (!! / ?? / …) overlay for 1.8 seconds after a move.
#[derive(Resource, Default)]
pub struct QualityBadgeState {
    pub quality: nimzovich_engine::MoveQuality,
    /// Remaining display time in seconds
    pub timer: f32,
    /// 0.0 = invisible, 1.0 = fully visible (fade in 0.2 → hold → fade out 0.2)
    pub alpha: f32,
}

#[derive(Message, Default)] pub struct BlunderFlash;
#[derive(Message, Default)] pub struct BrilliantGlow;
#[derive(Message, Default)] pub struct CheckmateFlash;

// ─────────────────────────────────────────────────────────────────────────────
// System: load PGN annotations per-ply and fire cinematic events
// ─────────────────────────────────────────────────────────────────────────────

/// When the replay advances to a new ply, load embedded PGN annotations
/// (arrows, highlights, quality badge) and dispatch cinematic events.
pub fn load_pgn_annotations_system(
    mut replay: ResMut<PgnReplayState>,
    pgn: Option<Res<ParsedPgnGameResource>>,
    mut annotations: ResMut<ReplayAnnotations>,
    mut badge: ResMut<QualityBadgeState>,
    mut blunder_ev: MessageWriter<BlunderFlash>,
    mut brilliant_ev: MessageWriter<BrilliantGlow>,
    mut checkmate_ev: MessageWriter<CheckmateFlash>,
) {
    let Some(pgn) = pgn else { return };
    let ply = replay.engine_ply;

    if ply == replay.last_annotation_ply { return; }
    replay.last_annotation_ply = ply;

    // Clear existing annotations before loading new ones
    annotations.arrows.clear();
    annotations.highlights.clear();
    annotations.dirty = true;

    let ann = pgn.inner.per_ply_annotations.get(ply);
    let quality = ann.map(|a| a.quality).unwrap_or_default();

    // Copy arrows + highlights from PGN
    if let Some(a) = ann {
        annotations.arrows.extend_from_slice(&a.arrows);
        annotations.highlights.extend_from_slice(&a.highlights);
        if !a.arrows.is_empty() || !a.highlights.is_empty() {
            annotations.dirty = true;
        }
    }

    // Quality badge
    use nimzovich_engine::MoveQuality;
    if quality != MoveQuality::Normal {
        badge.quality = quality;
        badge.timer = 1.8;
        badge.alpha = 0.0;
    }

    // Cinematic effects
    let (slow, timer) = match quality {
        MoveQuality::Brilliant => { brilliant_ev.write(BrilliantGlow); (0.35, 2.0) }
        MoveQuality::Good      => (0.7, 1.0),
        MoveQuality::Blunder   => { blunder_ev.write(BlunderFlash);   (0.45, 2.0) }
        MoveQuality::Mistake   => { blunder_ev.write(BlunderFlash);   (0.6, 1.5) }
        _                      => (1.0, 0.0),
    };
    replay.slow_factor = slow;
    replay.cinematic_timer = timer;

    // Checkmate flash (detected via game state)
    use nimzovich_engine::{get_game_state, STATE_CHECKMATE, COLOR_WHITE, COLOR_BLACK};
    let color = if replay.engine.move_counter % 2 == 0 { COLOR_WHITE } else { COLOR_BLACK };
    let state = get_game_state(&mut replay.engine, color);
    if state == STATE_CHECKMATE {
        checkmate_ev.write(CheckmateFlash);
        replay.slow_factor = 0.08;
        replay.cinematic_timer = 4.0;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// System: tick cinematic timers (Update)
// ─────────────────────────────────────────────────────────────────────────────

/// Ticks CinematicEffect flash alpha and PgnReplayState cinematic / slow-factor.
pub fn cinematic_tick_system(
    time: Res<Time>,
    mut effect: ResMut<CinematicEffect>,
    mut badge: ResMut<QualityBadgeState>,
    mut replay: ResMut<PgnReplayState>,
    mut blunder_ev: MessageReader<BlunderFlash>,
    mut brilliant_ev: MessageReader<BrilliantGlow>,
    mut checkmate_ev: MessageReader<CheckmateFlash>,
) {
    let dt = time.delta_secs();

    for _ in blunder_ev.read() {
        effect.flash_color = [0.86, 0.12, 0.12];
        effect.flash_alpha = 0.55;
        effect.flash_decay = 0.6;
    }
    for _ in brilliant_ev.read() {
        effect.flash_color = [1.0, 0.78, 0.12];
        effect.flash_alpha = 0.45;
        effect.flash_decay = 0.5;
    }
    for _ in checkmate_ev.read() {
        effect.flash_color = [1.0, 1.0, 1.0];
        effect.flash_alpha = 0.70;
        effect.flash_decay = 0.22;
    }

    // Decay flash
    if effect.flash_alpha > 0.0 {
        effect.flash_alpha = (effect.flash_alpha - effect.flash_decay * dt).max(0.0);
    }

    // Tick quality badge
    if badge.timer > 0.0 {
        badge.timer -= dt;
        let total = 1.8_f32;
        let remaining = badge.timer.max(0.0);
        let elapsed = total - remaining;
        badge.alpha = if elapsed < 0.2 {
            elapsed / 0.2
        } else if remaining < 0.2 {
            remaining / 0.2
        } else {
            1.0
        };
        if badge.timer <= 0.0 {
            badge.alpha = 0.0;
        }
    }

    // Tick slow-motion recovery
    if replay.cinematic_timer > 0.0 {
        replay.cinematic_timer -= dt;
        if replay.cinematic_timer <= 0.0 {
            replay.cinematic_timer = 0.0;
            replay.slow_factor = 1.0;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// System: cinematic flash overlay (EguiPrimaryContextPass)
// ─────────────────────────────────────────────────────────────────────────────

/// Renders the full-screen color flash overlay for blunder/brilliant/checkmate.
pub fn cinematic_effect_system(
    mut contexts: EguiContexts,
    effect: Res<CinematicEffect>,
    game_mode: Res<GameMode>,
) {
    if *game_mode != GameMode::PgnReplay { return; }
    if effect.flash_alpha < 0.01 { return; }

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };
    let [r, g, b] = effect.flash_color;
    let a = (effect.flash_alpha * 255.0) as u8;

    egui::Area::new(egui::Id::new("cinematic_flash"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let screen = ctx.screen_rect();
            let painter = ui.painter_at(screen);
            painter.rect_filled(
                screen,
                0.0,
                egui::Color32::from_rgba_unmultiplied(
                    (r * 255.0) as u8, (g * 255.0) as u8, b as u8, a,
                ),
            );
        });
}

// ─────────────────────────────────────────────────────────────────────────────
// System: quality badge overlay (EguiPrimaryContextPass)
// ─────────────────────────────────────────────────────────────────────────────

/// Draws the !! / ?? / ?! quality badge centred on screen for 1.8 s.
pub fn quality_badge_system(
    mut contexts: EguiContexts,
    badge: Res<QualityBadgeState>,
    game_mode: Res<GameMode>,
) {
    if *game_mode != GameMode::PgnReplay { return; }
    if badge.alpha < 0.01 { return; }

    use nimzovich_engine::MoveQuality;
    let (symbol, color_rgb) = match badge.quality {
        MoveQuality::Brilliant   => ("!!", [255u8, 200, 30]),
        MoveQuality::Good        => ("!",  [100, 220, 100]),
        MoveQuality::Interesting => ("!?", [180, 100, 220]),
        MoveQuality::Dubious     => ("?!", [220, 140, 20]),
        MoveQuality::Mistake     => ("?",  [220, 120, 30]),
        MoveQuality::Blunder     => ("??", [220, 40, 40]),
        MoveQuality::Normal      => return,
    };

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    let a = (badge.alpha * 230.0) as u8;
    let [r, g, b] = color_rgb;

    egui::Area::new(egui::Id::new("quality_badge"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let screen = ctx.screen_rect();
            let cx = screen.center().x;
            let cy = screen.center().y - 60.0;
            let bg_rect = egui::Rect::from_center_size(
                egui::Pos2::new(cx, cy),
                egui::Vec2::new(100.0, 56.0),
            );
            let painter = ui.painter_at(screen);
            painter.rect_filled(
                bg_rect,
                10.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, (badge.alpha * 180.0) as u8),
            );
            painter.text(
                bg_rect.center(),
                egui::Align2::CENTER_CENTER,
                symbol,
                egui::FontId::proportional(36.0),
                egui::Color32::from_rgba_unmultiplied(r, g, b, a),
            );
        });
}

// ─────────────────────────────────────────────────────────────────────────────
// System: hook text overlay (EguiPrimaryContextPass)
// ─────────────────────────────────────────────────────────────────────────────

/// Draws the creator-authored hook text overlay for the current ply.
pub fn hook_text_system(
    mut contexts: EguiContexts,
    mut shorts: ResMut<crate::game::shorts_state::ShortsState>,
    replay: Res<PgnReplayState>,
    game_mode: Res<GameMode>,
    time: Res<Time>,
) {
    if *game_mode != GameMode::PgnReplay { return; }

    // Tick hook text alpha (fade in over 0.3s when a hook text is present)
    let has_hook = shorts.hook_texts.contains_key(&replay.engine_ply);
    if has_hook {
        shorts.hook_text_alpha = (shorts.hook_text_alpha + time.delta_secs() / 0.3).min(1.0);
    } else {
        shorts.hook_text_alpha = (shorts.hook_text_alpha - time.delta_secs() / 0.2).max(0.0);
    }
    if shorts.hook_text_alpha < 0.01 { return; }

    let hook = match shorts.hook_texts.get(&replay.engine_ply) {
        Some(h) => h.clone(),
        None => return,
    };

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    use crate::game::shorts_state::HookStyle;
    let a = (shorts.hook_text_alpha * 255.0) as u8;
    let screen = ctx.screen_rect();

    egui::Area::new(egui::Id::new("hook_text"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter_at(screen);
            match hook.style {
                HookStyle::TopBold => {
                    let pos = egui::Pos2::new(screen.center().x, 28.0);
                    painter.text(
                        pos + egui::Vec2::new(2.0, 2.0),
                        egui::Align2::CENTER_CENTER,
                        &hook.text,
                        egui::FontId::proportional(28.0),
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, (a as f32 * 0.6) as u8),
                    );
                    painter.text(
                        pos,
                        egui::Align2::CENTER_CENTER,
                        &hook.text,
                        egui::FontId::proportional(28.0),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, a),
                    );
                }
                HookStyle::BottomCaption => {
                    let bar = egui::Rect::from_min_size(
                        egui::Pos2::new(0.0, screen.max.y - 52.0),
                        egui::Vec2::new(screen.width(), 52.0),
                    );
                    painter.rect_filled(
                        bar,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, (a as f32 * 0.7) as u8),
                    );
                    painter.text(
                        bar.center(),
                        egui::Align2::CENTER_CENTER,
                        &hook.text,
                        egui::FontId::proportional(20.0),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, a),
                    );
                }
                HookStyle::CenterDramatic => {
                    let bg = egui::Rect::from_center_size(
                        screen.center(),
                        egui::Vec2::new(screen.width() * 0.8, 80.0),
                    );
                    painter.rect_filled(
                        bg,
                        8.0,
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, (a as f32 * 0.75) as u8),
                    );
                    painter.text(
                        bg.center(),
                        egui::Align2::CENTER_CENTER,
                        &hook.text,
                        egui::FontId::proportional(32.0),
                        egui::Color32::from_rgba_unmultiplied(255, 220, 60, a),
                    );
                }
            }
        });
}

// ─────────────────────────────────────────────────────────────────────────────
// System: sequence capture (Update)
// ─────────────────────────────────────────────────────────────────────────────

/// When capture mode is active, waits for the tween settle delay, fires
/// ScreenshotRequested, then advances the ply until the sequence is complete.
pub fn capture_sequence_system(
    time: Res<Time>,
    mut shorts: ResMut<crate::game::shorts_state::ShortsState>,
    mut replay: ResMut<PgnReplayState>,
    mut screenshot_writer: MessageWriter<ScreenshotRequested>,
    pgn: Option<Res<ParsedPgnGameResource>>,
) {
    let Some(seq) = shorts.capture_mode.as_mut() else { return };
    let Some(_pgn) = pgn else { return };

    seq.timer += time.delta_secs();
    if seq.timer < seq.delay_secs { return; }
    seq.timer = 0.0;

    // Fire screenshot
    screenshot_writer.write(ScreenshotRequested);

    // Advance ply
    seq.current += 1;
    if seq.current > seq.to_ply {
        info!("[SHORTS] Capture sequence complete — {} frames", seq.to_ply - seq.from_ply + 1);
        shorts.capture_mode = None;
        return;
    }

    replay.current_ply = seq.current;
    replay.position_dirty = true;
    replay.paused = true;
}
