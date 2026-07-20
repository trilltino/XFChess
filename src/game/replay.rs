//! PGN Replay module — playback controls for loaded chess games.
//!
//! Provides:
//! - Auto-advance with configurable speed
//! - Step forward / backward
//! - Jump to any move
//! - 2D/3D view toggle support (both render from the same ECS `Piece` components)
//!
//! The replay uses an internal `nimzovich_engine::Game` to apply moves and
//! stores FEN snapshots after every ply for instant backward navigation.

use crate::core::{DespawnOnExit, GameMode, GameState};
use crate::engine::board_state::ChessEngine;
use crate::game::components::{HasMoved, PieceMoveAnimation};
use crate::game::replay_shorts::{PuzzleOverlay, ReplayAnnotations, ScreenshotRequested};
use crate::game::shorts_state::{ContentTier, HookStyle, HookText, ShortsState};
use crate::game::view_mode::ViewMode;
use crate::multiplayer::traits::MessageWriter;
use crate::rendering::pieces::{
    Piece, Piece2DVisual, Piece3DVisual, PieceColor, PieceMeshes, PieceSpriteHandles, PieceType,
    PiecesSpawned, PIECE_ON_BOARD_Y,
};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use nimzovich_engine::{
    do_move_with_promo, game_from_fen_no_tt, game_to_fen, new_game_no_tt, san_to_move,
};

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Bevy resource wrapping the parsed PGN game.
#[derive(Resource, Debug, Clone)]
pub struct ParsedPgnGameResource {
    pub inner: nimzovich_engine::ParsedPgnGame,
    /// When true, the replay UI renders the eval sparkline overlay.
    pub show_eval_graph: bool,
    /// When true, the move list hides moves after the current ply (puzzle mode).
    pub puzzle_mode: bool,
    /// When true, the answer has been revealed in puzzle mode.
    pub puzzle_revealed: bool,
}

/// Tracks the playback state of a loaded PGN replay.
#[derive(Resource)]
pub struct PgnReplayState {
    /// Internal engine — always at the position of `current_ply`.
    pub engine: nimzovich_engine::Game,
    /// FEN string after each ply. Index 0 = start, index N = after ply N.
    pub fen_snapshots: Vec<String>,
    /// Current ply (half-move). 0 = start, moves[0] = ply 1, etc.
    pub current_ply: usize,
    /// Whether playback is paused.
    pub paused: bool,
    /// Seconds between auto-advances.
    pub speed: f32,
    /// Timer for auto-advance.
    pub timer: Timer,
    /// Has the board been spawned yet?
    pub board_ready: bool,
    /// Whether the position changed this frame (triggers re-sync).
    pub position_dirty: bool,

    // ── Cinematic / shorts ──
    /// Board state from the previous ply (used to diff and tween the moved piece).
    pub prev_board: [i8; 64],
    /// The ply index that the engine was last rebuilt to (for diffing).
    pub engine_ply: usize,
    /// True when the next board spawn should inject a PieceMoveAnimation tween.
    pub animate_next_advance: bool,
    /// Slow-motion factor for piece tweens: 1.0 = normal, <1.0 = slow.
    pub slow_factor: f32,
    /// Remaining seconds of cinematic slow-motion.
    pub cinematic_timer: f32,
    /// Ply index for which annotations were last loaded (usize::MAX = never).
    pub last_annotation_ply: usize,

    // ── In-replayer PGN paste ──
    pub show_pgn_input: bool,
    pub pgn_input_text: String,
    pub pgn_input_error: Option<String>,
}

impl Default for PgnReplayState {
    fn default() -> Self {
        Self {
            engine: new_game_no_tt(),
            fen_snapshots: vec![
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()
            ],
            current_ply: 0,
            paused: true,
            speed: 1.0,
            timer: Timer::from_seconds(1.0, TimerMode::Once),
            board_ready: false,
            position_dirty: false,
            prev_board: [0i8; 64],
            engine_ply: 0,
            animate_next_advance: false,
            slow_factor: 1.0,
            cinematic_timer: 0.0,
            last_annotation_ply: usize::MAX,
            show_pgn_input: false,
            pgn_input_text: String::new(),
            pgn_input_error: None,
        }
    }
}

impl PgnReplayState {
    pub fn total_plies(&self) -> usize {
        self.fen_snapshots.len().saturating_sub(1)
    }
}

// ---------------------------------------------------------------------------
// Setup / Cleanup
// ---------------------------------------------------------------------------

/// Run when entering `InGame` with `GameMode::PgnReplay`.
/// Parses the SAN moves into FEN snapshots and initialises the replay engine.
pub fn setup_replay(
    parsed_pgn: Option<Res<ParsedPgnGameResource>>,
    mut replay: ResMut<PgnReplayState>,
    mut engine: ResMut<ChessEngine>,
    mut pieces_spawned: ResMut<PiecesSpawned>,
) {
    let Some(pgn) = parsed_pgn else {
        warn!("[REPLAY] setup_replay called but no ParsedPgnGameResource present");
        return;
    };

    info!(
        "[REPLAY] Setting up replay: {} moves",
        pgn.inner.moves.len()
    );

    // Reset replay state
    *replay = PgnReplayState::default();
    replay.engine = new_game_no_tt();

    // Pre-generate all FEN snapshots by applying moves sequentially
    let mut temp_engine = new_game_no_tt();
    replay.fen_snapshots.clear();
    replay
        .fen_snapshots
        .push("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string());

    for (i, san) in pgn.inner.moves.iter().enumerate() {
        match san_to_move(&mut temp_engine, san) {
            Ok((src, dst, promo)) => {
                do_move_with_promo(&mut temp_engine, src, dst, true, promo);
                let fen = engine_to_fen(&temp_engine);
                replay.fen_snapshots.push(fen);
            }
            Err(e) => {
                warn!(
                    "[REPLAY] Failed to resolve move {} '{}': {:?}",
                    i + 1,
                    san,
                    e
                );
                break;
            }
        }
    }

    info!(
        "[REPLAY] Generated {} FEN snapshots for {} plies",
        replay.fen_snapshots.len(),
        pgn.inner.moves.len()
    );

    // Sync the main ChessEngine to starting position
    engine.set_from_fen(&replay.fen_snapshots[0]).ok();

    // Mark board as needing spawn
    replay.board_ready = false;
    replay.position_dirty = true;
    pieces_spawned.spawned = false;

    info!("[REPLAY] Setup complete — ready to spawn board");
}

/// Despawn all pieces when exiting replay.
pub fn cleanup_replay(mut commands: Commands, pieces: Query<Entity, With<Piece>>) {
    for entity in pieces.iter() {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<PgnReplayState>();
    commands.remove_resource::<ParsedPgnGameResource>();
    info!("[REPLAY] Cleaned up replay resources");
}

// ---------------------------------------------------------------------------
// Playback Systems
// ---------------------------------------------------------------------------

/// Auto-advance the replay when playing and timer fires.
pub fn replay_auto_advance_system(
    mut replay: ResMut<PgnReplayState>,
    parsed_pgn: Option<Res<ParsedPgnGameResource>>,
    time: Res<Time>,
) {
    let Some(pgn) = parsed_pgn else { return };
    if replay.paused {
        return;
    }
    if replay.current_ply >= pgn.inner.moves.len() {
        replay.paused = true;
        return;
    }

    replay.timer.tick(time.delta());
    if replay.timer.just_finished() {
        replay.current_ply += 1;
        replay.position_dirty = true;
        replay.timer = Timer::from_seconds(replay.speed, TimerMode::Once);
    }
}

/// Apply the current ply to the engine and mark position dirty.
pub fn replay_apply_move_system(
    mut replay: ResMut<PgnReplayState>,
    parsed_pgn: Option<Res<ParsedPgnGameResource>>,
) {
    if !replay.position_dirty {
        return;
    }
    replay.position_dirty = false;

    let Some(pgn) = parsed_pgn else { return };

    // Clamp to valid range
    let target_ply = replay.current_ply.min(pgn.inner.moves.len());

    // Save board state BEFORE rebuilding so we can diff for tween animation
    replay.prev_board = replay.engine.board;
    // Single forward step → inject tween; jump or backward → full respawn only
    replay.animate_next_advance = target_ply == replay.engine_ply + 1;
    replay.engine_ply = target_ply;

    // If we have a FEN snapshot, rebuild from it (handles both forward and backward)
    if target_ply < replay.fen_snapshots.len() {
        let fen = replay.fen_snapshots[target_ply].clone();
        replay.engine = game_from_fen_no_tt(&fen);
        // Trigger piece re-spawn so the board visuals update.
        replay.board_ready = false;
    } else {
        // Shouldn't happen if snapshots were generated correctly
        warn!("[REPLAY] Missing FEN snapshot for ply {}", target_ply);
    }
}

/// Sync the replay engine to the main ChessEngine resource so the board
/// rendering (2D and 3D) sees the correct position.
pub fn replay_sync_engine_system(replay: Res<PgnReplayState>, mut engine: ResMut<ChessEngine>) {
    let fen = engine_to_fen(&replay.engine);
    if engine.fen != fen {
        engine.set_from_fen(&fen).ok();
    }
}

// ---------------------------------------------------------------------------
// Piece Spawning from Engine Board
// ---------------------------------------------------------------------------

/// Despawn all existing pieces and respawn from the current engine board.
pub fn replay_spawn_pieces_system(
    mut commands: Commands,
    mut replay: ResMut<PgnReplayState>,
    mut engine: ResMut<ChessEngine>,
    asset_server: Res<AssetServer>,
    piece_meshes: Res<PieceMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut pieces_spawned: ResMut<PiecesSpawned>,
    existing_pieces: Query<Entity, With<Piece>>,
    sprite_handles: Option<Res<PieceSpriteHandles>>,
) {
    if replay.board_ready {
        return;
    }

    // Wait for meshes to load
    let meshes_to_check = piece_meshes.all_ids();
    for mesh_id in meshes_to_check.iter() {
        match asset_server.load_state(*mesh_id) {
            bevy::asset::LoadState::Loaded => {}
            _ => return,
        }
    }

    info!("[REPLAY] Spawning pieces from engine board");

    // Despawn existing pieces
    for entity in existing_pieces.iter() {
        commands.entity(entity).despawn();
    }

    // Copy state we need before mutably borrowing replay later
    let animate = replay.animate_next_advance;
    let engine_ply = replay.engine_ply;
    let slow_factor = replay.slow_factor;
    let prev_board = replay.prev_board;
    let curr_board = replay.engine.board;

    // Spawn pieces from engine board; collect entity at each square for tween injection
    let mut entity_at_sq: std::collections::HashMap<usize, Entity> =
        std::collections::HashMap::new();

    for sq in 0..64usize {
        let piece_id = curr_board[sq];
        if piece_id == 0 {
            continue;
        }
        let file = (sq % 8) as u8;
        let rank = (sq / 8) as u8;
        let color = if piece_id > 0 {
            PieceColor::White
        } else {
            PieceColor::Black
        };
        let piece_type = engine_id_to_piece_type(piece_id.abs());

        let piece_material = if color == PieceColor::White {
            materials.add(crate::rendering::pieces::pieces::white_piece_material())
        } else {
            materials.add(crate::rendering::pieces::pieces::black_piece_material())
        };

        let entity = spawn_piece_at_replay(
            &mut commands,
            &piece_meshes,
            piece_material,
            color,
            piece_type,
            (file, rank),
            Vec3::ZERO,
            &sprite_handles,
        );
        entity_at_sq.insert(sq, entity);
    }

    // If this was a single forward advance, inject a PieceMoveAnimation tween
    if animate && engine_ply > 0 {
        let mut src_sq: Option<usize> = None;
        let mut dst_sq: Option<usize> = None;
        for sq in 0..64usize {
            let p = prev_board[sq];
            let c = curr_board[sq];
            if p != 0 && c == 0 && src_sq.is_none() {
                src_sq = Some(sq);
            }
            // Destination: piece arrived (was empty) or captured (colour flipped)
            if c != 0 && p != c && dst_sq.is_none() {
                if p == 0 || (p != 0 && p.signum() != c.signum()) {
                    dst_sq = Some(sq);
                }
            }
        }
        if let (Some(src), Some(dst)) = (src_sq, dst_sq) {
            // World X is mirrored (7 - file), matching spawn_piece_at_replay /
            // execute_move's PieceMoveAnimation targets — see pieces.rs:484.
            let src_world = Vec3::new(7.0 - (src % 8) as f32, PIECE_ON_BOARD_Y, (src / 8) as f32);
            let dst_world = Vec3::new(7.0 - (dst % 8) as f32, PIECE_ON_BOARD_Y, (dst / 8) as f32);
            if let Some(&ent) = entity_at_sq.get(&dst) {
                let duration = 0.3 / slow_factor.max(0.05);
                commands
                    .entity(ent)
                    .insert(PieceMoveAnimation::new(src_world, dst_world, duration));
            }
        }
    }
    replay.animate_next_advance = false;

    // Sync engine to ECS
    engine.refresh_position();

    replay.board_ready = true;
    pieces_spawned.spawned = true;
    info!("[REPLAY] Pieces spawned successfully");
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

/// Replay control bar and move list overlay.
pub fn replay_ui_system(
    mut contexts: EguiContexts,
    mut replay: ResMut<PgnReplayState>,
    mut parsed_pgn: Option<ResMut<ParsedPgnGameResource>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut view_mode: ResMut<ViewMode>,
    game_mode: Res<GameMode>,
    eval_history: Option<Res<crate::ui::game::game_2d::EvalHistory>>,
    mut puzzle: ResMut<PuzzleOverlay>,
    mut annotations: ResMut<ReplayAnnotations>,
    mut screenshot_writer: MessageWriter<ScreenshotRequested>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut shorts: ResMut<ShortsState>,
    mut commands: Commands,
) {
    if *game_mode != GameMode::PgnReplay {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // No PGN loaded yet — show the paste dialog centred on screen.
    if parsed_pgn.is_none() {
        egui::Window::new("pgn_load_overlay")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::Vec2::new(480.0, 320.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(22, 22, 22, 245),
                corner_radius: egui::CornerRadius::same(6),
                stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(60, 60, 60)),
                inner_margin: egui::Margin::same(20),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("PGN Replay").size(18.0).color(egui::Color32::WHITE).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Exit").clicked() {
                            next_state.set(GameState::MainMenu);
                        }
                    });
                });
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Paste a PGN game and click Load.").size(11.0).color(egui::Color32::from_rgb(160, 170, 190)));
                ui.add_space(8.0);
                egui::ScrollArea::vertical().max_height(170.0).show(ui, |ui| {
                    ui.add_sized(
                        [440.0, 160.0],
                        egui::TextEdit::multiline(&mut replay.pgn_input_text)
                            .font(egui::TextStyle::Monospace)
                            .hint_text("[Event \"?\"]\n[White \"Player1\"]\n[Black \"Player2\"]\n\n1. e4 e5 2. Nf3 ..."),
                    );
                });
                if let Some(ref err) = replay.pgn_input_error.clone() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("Error: {}", err)).size(10.5).color(egui::Color32::from_rgb(230, 100, 80)));
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let can_load = !replay.pgn_input_text.trim().is_empty();
                    if ui.add_enabled(
                        can_load,
                        egui::Button::new(egui::RichText::new("Load & Play").size(13.0).color(egui::Color32::WHITE).strong())
                            .fill(egui::Color32::from_rgb(50, 120, 60))
                            .corner_radius(4.0)
                            .min_size(egui::Vec2::new(120.0, 32.0)),
                    ).clicked() {
                        match nimzovich_engine::parse_pgn(&replay.pgn_input_text) {
                            Ok(pgn) => {
                                // Build FEN snapshots inline
                                let mut temp = new_game_no_tt();
                                let mut snapshots = vec!["rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()];
                                for (i, san) in pgn.moves.iter().enumerate() {
                                    match san_to_move(&mut temp, san) {
                                        Ok((src, dst, promo)) => {
                                            do_move_with_promo(&mut temp, src, dst, true, promo);
                                            snapshots.push(game_to_fen(&temp));
                                        }
                                        Err(e) => {
                                            warn!("[REPLAY] Failed move {} '{}': {:?}", i + 1, san, e);
                                            break;
                                        }
                                    }
                                }
                                replay.fen_snapshots = snapshots;
                                replay.current_ply = 0;
                                replay.board_ready = false;
                                replay.position_dirty = true;
                                replay.paused = true;
                                replay.pgn_input_error = None;
                                replay.pgn_input_text.clear();
                                commands.insert_resource(ParsedPgnGameResource {
                                    inner: pgn,
                                    show_eval_graph: false,
                                    puzzle_mode: false,
                                    puzzle_revealed: false,
                                });
                            }
                            Err(e) => {
                                replay.pgn_input_error = Some(format!("{:?}", e));
                            }
                        }
                    }
                });
            });
        return;
    }

    let Some(ref pgn) = parsed_pgn else { return };

    // --- Eval sparkline (shown when analyze mode active) ---
    let show_graph = pgn.show_eval_graph;
    if show_graph {
        if let Some(eh) = eval_history.as_ref() {
            if !eh.scores.is_empty() {
                egui::TopBottomPanel::bottom("replay_eval_graph")
                    .exact_height(52.0)
                    .frame(egui::Frame {
                        fill: egui::Color32::from_rgba_unmultiplied(20, 20, 20, 230),
                        inner_margin: egui::Margin::symmetric(8, 4),
                        ..Default::default()
                    })
                    .show(ctx, |ui| {
                        let scores = &eh.scores;
                        let n = scores.len();
                        let avail = ui.available_width();
                        let bar_w = (avail / n as f32).max(2.0).min(12.0);
                        let total_w = bar_w * n as f32;
                        let height = 40.0;
                        let (rect, _) = ui.allocate_exact_size(
                            egui::Vec2::new(total_w, height),
                            egui::Sense::hover(),
                        );
                        let painter = ui.painter();
                        let mid_y = rect.center().y;

                        painter.line_segment(
                            [
                                egui::Pos2::new(rect.left(), mid_y),
                                egui::Pos2::new(rect.right(), mid_y),
                            ],
                            egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
                        );

                        for (i, &score) in scores.iter().enumerate() {
                            let x = rect.left() + i as f32 * bar_w;
                            let clamped = score.clamp(-800, 800) as f32;
                            let frac = clamped / 800.0;
                            let bar_h = (frac.abs() * (height / 2.0 - 2.0)).max(1.0);
                            let color = if score >= 0 {
                                egui::Color32::from_rgb(200, 230, 200)
                            } else {
                                egui::Color32::from_rgb(80, 80, 80)
                            };
                            let top = if score >= 0 { mid_y - bar_h } else { mid_y };
                            let bot = if score >= 0 { mid_y } else { mid_y + bar_h };
                            painter.rect_filled(
                                egui::Rect::from_min_max(
                                    egui::Pos2::new(x + 1.0, top),
                                    egui::Pos2::new(x + bar_w - 1.0, bot),
                                ),
                                0.0,
                                color,
                            );
                        }

                        // Current ply marker
                        let cur = replay
                            .current_ply
                            .saturating_sub(1)
                            .min(n.saturating_sub(1));
                        let cx = rect.left() + cur as f32 * bar_w + bar_w / 2.0;
                        painter.line_segment(
                            [
                                egui::Pos2::new(cx, rect.top()),
                                egui::Pos2::new(cx, rect.bottom()),
                            ],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255)),
                        );
                    });
            }
        }
    }

    // Ctrl+S shortcut for screenshot
    if keyboard.pressed(KeyCode::ControlLeft) && keyboard.just_pressed(KeyCode::KeyS) {
        screenshot_writer.write(ScreenshotRequested);
    }

    // --- Bottom control bar ---
    egui::TopBottomPanel::bottom("replay_controls")
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240),
            inner_margin: egui::Margin::symmetric(12, 8),
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Navigation buttons
                let btn = |ui: &mut egui::Ui, label: &str| {
                    ui.add_sized(
                        [40.0, 28.0],
                        egui::Button::new(egui::RichText::new(label).size(14.0).strong())
                            .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
                            .corner_radius(4.0),
                    )
                };

                if btn(ui, "|<<").clicked() {
                    replay.current_ply = 0;
                    replay.position_dirty = true;
                    replay.paused = true;
                    annotations.arrows.clear();
                    annotations.highlights.clear();
                    annotations.dirty = true;
                }
                if btn(ui, "<").clicked() {
                    if replay.current_ply > 0 {
                        replay.current_ply -= 1;
                        replay.position_dirty = true;
                    }
                    replay.paused = true;
                    annotations.arrows.clear();
                    annotations.highlights.clear();
                    annotations.dirty = true;
                }

                // Play / Pause
                let play_label = if replay.paused { "▶" } else { "⏸" };
                if btn(ui, play_label).clicked() {
                    replay.paused = !replay.paused;
                    if !replay.paused {
                        replay.timer = Timer::from_seconds(replay.speed, TimerMode::Once);
                    }
                }

                if btn(ui, ">").clicked() {
                    if let Some(ref pgn_res) = parsed_pgn {
                        if replay.current_ply < pgn_res.inner.moves.len() {
                            // In puzzle mode don't advance past puzzle ply unless revealed
                            let blocked = puzzle.enabled
                                && !puzzle.revealed
                                && replay.current_ply >= puzzle.puzzle_ply;
                            if !blocked {
                                replay.current_ply += 1;
                                replay.position_dirty = true;
                            }
                        }
                    }
                    replay.paused = true;
                    annotations.arrows.clear();
                    annotations.highlights.clear();
                    annotations.dirty = true;
                }
                if btn(ui, ">>|").clicked() {
                    if let Some(ref pgn_res) = parsed_pgn {
                        if !puzzle.enabled || puzzle.revealed {
                            replay.current_ply = pgn_res.inner.moves.len();
                            replay.position_dirty = true;
                        }
                    }
                    replay.paused = true;
                    annotations.arrows.clear();
                    annotations.highlights.clear();
                    annotations.dirty = true;
                }

                ui.add_space(12.0);

                // Speed slider
                ui.label(
                    egui::RichText::new("Speed:")
                        .size(12.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );
                let mut speed_label = replay.speed;
                ui.add_sized(
                    [100.0, 20.0],
                    egui::Slider::new(&mut speed_label, 0.2..=4.0)
                        .step_by(0.1)
                        .show_value(false),
                );
                if (speed_label - replay.speed).abs() > 0.01 {
                    replay.speed = speed_label;
                    replay.timer = Timer::from_seconds(replay.speed, TimerMode::Once);
                }
                ui.label(
                    egui::RichText::new(format!("{:.1}s", replay.speed))
                        .size(11.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );

                ui.add_space(12.0);

                // 2D/3D toggle
                let view_label = match *view_mode {
                    ViewMode::Standard2D => "3D",
                    ViewMode::Standard3D => "2D",
                    #[cfg(feature = "templeos")]
                    ViewMode::TempleOS => "3D",
                };
                if ui
                    .add_sized(
                        [50.0, 28.0],
                        egui::Button::new(egui::RichText::new(view_label).size(12.0).strong())
                            .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
                            .corner_radius(4.0),
                    )
                    .clicked()
                {
                    view_mode.toggle();
                    // Rebuild 3D annotations on view switch
                    annotations.dirty = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [100.0, 28.0],
                            egui::Button::new(
                                egui::RichText::new("Exit to Menu").size(12.0).strong(),
                            )
                            .fill(egui::Color32::from_rgb(120, 70, 70))
                            .corner_radius(4.0),
                        )
                        .clicked()
                    {
                        next_state.set(GameState::MainMenu);
                    }

                    ui.add_space(8.0);

                    // Screenshot button (Ctrl+S)
                    if ui
                        .add_sized(
                            [36.0, 28.0],
                            egui::Button::new(egui::RichText::new("📷").size(14.0))
                                .fill(egui::Color32::from_rgba_unmultiplied(40, 80, 60, 220))
                                .corner_radius(4.0),
                        )
                        .on_hover_text("Screenshot (Ctrl+S)")
                        .clicked()
                    {
                        screenshot_writer.write(ScreenshotRequested);
                    }

                    ui.add_space(4.0);

                    // Puzzle mode toggle
                    let puzzle_col = if puzzle.enabled {
                        egui::Color32::from_rgb(200, 140, 20)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200)
                    };
                    if ui
                        .add_sized(
                            [36.0, 28.0],
                            egui::Button::new(egui::RichText::new("🧩").size(14.0))
                                .fill(puzzle_col)
                                .corner_radius(4.0),
                        )
                        .on_hover_text("Puzzle mode — hide the answer move")
                        .clicked()
                    {
                        puzzle.enabled = !puzzle.enabled;
                        if puzzle.enabled {
                            puzzle.revealed = false;
                            puzzle.puzzle_ply = replay.current_ply;
                        }
                    }

                    ui.add_space(4.0);

                    // FEN input toggle
                    if ui
                        .add_sized(
                            [36.0, 28.0],
                            egui::Button::new(egui::RichText::new("FEN").size(11.0).strong())
                                .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
                                .corner_radius(4.0),
                        )
                        .on_hover_text("Load position from FEN")
                        .clicked()
                    {
                        puzzle.show_fen_input = !puzzle.show_fen_input;
                    }
                });
            });

            // FEN input row (shown when toggled)
            if puzzle.show_fen_input {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("FEN:")
                            .size(11.0)
                            .color(egui::Color32::LIGHT_GRAY),
                    );
                    let resp = ui.add_sized(
                        [ui.available_width() - 70.0, 22.0],
                        egui::TextEdit::singleline(&mut puzzle.fen_input)
                            .hint_text("Paste FEN here…")
                            .font(egui::FontId::monospace(11.0)),
                    );
                    if (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        || ui
                            .add_sized([60.0, 22.0], egui::Button::new("Load"))
                            .clicked()
                    {
                        use nimzovich_engine::game_from_fen_no_tt;
                        let fen = puzzle.fen_input.trim().to_string();
                        if !fen.is_empty() {
                            replay.engine = game_from_fen_no_tt(&fen);
                            replay.fen_snapshots = vec![fen.clone()];
                            replay.current_ply = 0;
                            replay.board_ready = false;
                            replay.position_dirty = true;
                            replay.paused = true;
                            puzzle.show_fen_input = false;
                            annotations.arrows.clear();
                            annotations.highlights.clear();
                            annotations.dirty = true;
                            info!("[SHORTS] Loaded FEN position: {}", fen);
                        }
                    }
                });
            }

            // Puzzle reveal row
            if puzzle.enabled && !puzzle.revealed {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("🤔 Puzzle — can you find the move?")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(255, 220, 60)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized(
                                [70.0, 22.0],
                                egui::Button::new(egui::RichText::new("Reveal").size(11.0))
                                    .fill(egui::Color32::from_rgb(80, 160, 80))
                                    .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            puzzle.revealed = true;
                            if let Some(ref pgn_res) = parsed_pgn {
                                // Advance to show the answer move
                                if puzzle.puzzle_ply < pgn_res.inner.moves.len() {
                                    replay.current_ply = puzzle.puzzle_ply + 1;
                                    replay.position_dirty = true;
                                }
                            }
                        }
                    });
                });
            }
        });

    // --- Move list panel (right side) ---
    let Some(pgn) = parsed_pgn else { return };
    let total = pgn.inner.moves.len();
    if total == 0 {
        return;
    }
    // In puzzle mode (unrevealed), only show moves up to the puzzle ply
    let visible_total = if puzzle.enabled && !puzzle.revealed {
        puzzle.puzzle_ply.min(total)
    } else {
        total
    };

    egui::SidePanel::right("replay_move_list")
        .min_width(200.0)
        .max_width(240.0)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(18, 18, 24, 230),
            inner_margin: egui::Margin::symmetric(10, 8),
            ..Default::default()
        })
        .show(ctx, |ui| {
            // PGN header (Lichess-style: White / Black / Result)
            if let Some(white) = pgn.inner.tag("White") {
                ui.label(
                    egui::RichText::new(format!("♔ {}", white))
                        .size(12.0)
                        .color(egui::Color32::from_gray(220)),
                );
            }
            if let Some(black) = pgn.inner.tag("Black") {
                ui.label(
                    egui::RichText::new(format!("♚ {}", black))
                        .size(12.0)
                        .color(egui::Color32::from_gray(160)),
                );
            }
            if !pgn.inner.result.is_empty() {
                ui.label(
                    egui::RichText::new(&pgn.inner.result)
                        .size(13.0)
                        .color(egui::Color32::GOLD)
                        .strong(),
                );
            }
            ui.add(egui::Separator::default().spacing(6.0));

            // Move list — Lichess 3-column grid: index | white | black
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    egui::Grid::new("replay_move_grid")
                        .num_columns(3)
                        .min_col_width(24.0)
                        .spacing([2.0, 1.0])
                        .show(ui, |ui| {
                            for move_num in 1..=((visible_total + 1) / 2) {
                                let white_idx = (move_num - 1) * 2;
                                let black_idx = white_idx + 1;

                                // Index column
                                ui.label(
                                    egui::RichText::new(format!("{}.", move_num))
                                        .size(11.0)
                                        .color(egui::Color32::GRAY),
                                );

                                // White move
                                if white_idx < visible_total {
                                    let is_current = replay.current_ply == white_idx + 1;
                                    let color = if is_current {
                                        egui::Color32::from_rgb(100, 200, 255)
                                    } else {
                                        egui::Color32::WHITE
                                    };
                                    let resp = ui.selectable_label(
                                        is_current,
                                        egui::RichText::new(&pgn.inner.moves[white_idx])
                                            .size(12.0)
                                            .color(color)
                                            .strong(),
                                    );
                                    if resp.clicked() {
                                        replay.current_ply = white_idx + 1;
                                        replay.position_dirty = true;
                                        replay.paused = true;
                                    }
                                } else {
                                    ui.label("");
                                }

                                // Black move
                                if black_idx < visible_total {
                                    let is_current = replay.current_ply == black_idx + 1;
                                    let color = if is_current {
                                        egui::Color32::from_rgb(100, 200, 255)
                                    } else {
                                        egui::Color32::from_gray(180)
                                    };
                                    let resp = ui.selectable_label(
                                        is_current,
                                        egui::RichText::new(&pgn.inner.moves[black_idx])
                                            .size(12.0)
                                            .color(color)
                                            .strong(),
                                    );
                                    if resp.clicked() {
                                        replay.current_ply = black_idx + 1;
                                        replay.position_dirty = true;
                                        replay.paused = true;
                                    }
                                } else if white_idx < visible_total {
                                    ui.label(
                                        egui::RichText::new("…")
                                            .size(12.0)
                                            .color(egui::Color32::DARK_GRAY),
                                    );
                                } else {
                                    ui.label("");
                                }

                                ui.end_row();
                            }
                        });

                    ui.add_space(6.0);

                    // Puzzle mode status indicator in move list
                    if puzzle.enabled {
                        let status = if puzzle.revealed {
                            "✅ Revealed"
                        } else {
                            "🧩 Puzzle"
                        };
                        ui.label(
                            egui::RichText::new(status)
                                .size(10.0)
                                .color(egui::Color32::from_rgb(255, 220, 60)),
                        );
                    }

                    ui.label(
                        egui::RichText::new(format!("Ply {}/{}", replay.current_ply, total))
                            .size(10.0)
                            .color(egui::Color32::DARK_GRAY),
                    );

                    // ── Shorts panel ──────────────────────────────────────
                    ui.add_space(8.0);
                    ui.add(egui::Separator::default().spacing(4.0));

                    // Content tier selector
                    ui.label(
                        egui::RichText::new("Content Tier")
                            .size(10.0)
                            .color(egui::Color32::from_gray(160))
                            .strong(),
                    );
                    ui.horizontal_wrapped(|ui| {
                        for tier in [
                            ContentTier::None,
                            ContentTier::Puzzle,
                            ContentTier::Blunder,
                            ContentTier::Highlight,
                            ContentTier::OpeningTrap,
                        ] {
                            let active = shorts.content_tier == tier;
                            let col = if active {
                                egui::Color32::from_rgb(60, 140, 200)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(50, 50, 50, 200)
                            };
                            if ui
                                .add_sized(
                                    [ui.available_width().min(88.0), 20.0],
                                    egui::Button::new(egui::RichText::new(tier.label()).size(10.0))
                                        .fill(col)
                                        .corner_radius(3.0),
                                )
                                .clicked()
                            {
                                shorts.content_tier = tier;
                                // Apply preset hook text if none already set for ply 0
                                let default_hook = tier.default_hook();
                                if !default_hook.is_empty() && !shorts.hook_texts.contains_key(&0) {
                                    shorts.hook_input = default_hook.to_string();
                                }
                            }
                        }
                    });

                    ui.add_space(4.0);

                    // Hook text editor
                    let hook_btn_label = if shorts.show_hook_editor {
                        "▲ Hook Text"
                    } else {
                        "▼ Hook Text"
                    };
                    if ui
                        .add_sized(
                            [ui.available_width(), 20.0],
                            egui::Button::new(egui::RichText::new(hook_btn_label).size(10.0))
                                .fill(egui::Color32::from_rgba_unmultiplied(40, 60, 40, 200))
                                .corner_radius(3.0),
                        )
                        .clicked()
                    {
                        shorts.show_hook_editor = !shorts.show_hook_editor;
                        if shorts.show_hook_editor {
                            // Pre-fill with existing hook for this ply
                            if let Some(h) = shorts.hook_texts.get(&replay.current_ply) {
                                shorts.hook_input = h.text.clone();
                            }
                        }
                    }
                    if shorts.show_hook_editor {
                        ui.add_space(2.0);
                        ui.add_sized(
                            [ui.available_width(), 36.0],
                            egui::TextEdit::multiline(&mut shorts.hook_input)
                                .hint_text("Hook text for this ply…")
                                .font(egui::FontId::proportional(10.0)),
                        );
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [50.0, 18.0],
                                    egui::Button::new(egui::RichText::new("Save").size(10.0))
                                        .fill(egui::Color32::from_rgb(40, 120, 40))
                                        .corner_radius(3.0),
                                )
                                .clicked()
                            {
                                let text = shorts.hook_input.trim().to_string();
                                if text.is_empty() {
                                    shorts.hook_texts.remove(&replay.current_ply);
                                } else {
                                    shorts.hook_texts.insert(
                                        replay.current_ply,
                                        HookText {
                                            text,
                                            style: HookStyle::TopBold,
                                        },
                                    );
                                }
                                shorts.show_hook_editor = false;
                            }
                            if ui
                                .add_sized(
                                    [50.0, 18.0],
                                    egui::Button::new(egui::RichText::new("Clear").size(10.0))
                                        .fill(egui::Color32::from_rgb(100, 40, 40))
                                        .corner_radius(3.0),
                                )
                                .clicked()
                            {
                                shorts.hook_texts.remove(&replay.current_ply);
                                shorts.hook_input.clear();
                            }
                        });
                    }

                    // Beat marker toggle for current ply
                    ui.add_space(4.0);
                    let has_beat = shorts.beat_markers.contains_key(&replay.current_ply);
                    let beat_col = if has_beat {
                        egui::Color32::from_rgb(200, 140, 20)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(50, 50, 50, 200)
                    };
                    if ui
                        .add_sized(
                            [ui.available_width(), 18.0],
                            egui::Button::new(
                                egui::RichText::new(if has_beat {
                                    "♩ Beat marked"
                                } else {
                                    "♩ Mark beat"
                                })
                                .size(10.0),
                            )
                            .fill(beat_col)
                            .corner_radius(3.0),
                        )
                        .clicked()
                    {
                        if has_beat {
                            shorts.beat_markers.remove(&replay.current_ply);
                        } else {
                            let next_beat = shorts.beat_markers.len() + 1;
                            shorts
                                .beat_markers
                                .insert(replay.current_ply, format!("beat_{}", next_beat));
                        }
                    }

                    // Sequence capture
                    ui.add_space(4.0);
                    let capturing = shorts.capture_mode.is_some();
                    if !capturing {
                        if ui
                            .add_sized(
                                [ui.available_width(), 18.0],
                                egui::Button::new(
                                    egui::RichText::new("🎬 Capture Sequence").size(10.0),
                                )
                                .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 80, 220))
                                .corner_radius(3.0),
                            )
                            .clicked()
                        {
                            shorts.show_beat_export = !shorts.show_beat_export;
                        }
                        if shorts.show_beat_export {
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("From:")
                                        .size(9.0)
                                        .color(egui::Color32::LIGHT_GRAY),
                                );
                                ui.add_sized(
                                    [36.0, 16.0],
                                    egui::TextEdit::singleline(&mut shorts.capture_from_input)
                                        .font(egui::FontId::monospace(9.0)),
                                );
                                ui.label(
                                    egui::RichText::new("To:")
                                        .size(9.0)
                                        .color(egui::Color32::LIGHT_GRAY),
                                );
                                ui.add_sized(
                                    [36.0, 16.0],
                                    egui::TextEdit::singleline(&mut shorts.capture_to_input)
                                        .font(egui::FontId::monospace(9.0)),
                                );
                            });
                            if ui
                                .add_sized(
                                    [ui.available_width(), 18.0],
                                    egui::Button::new(egui::RichText::new("Start").size(10.0))
                                        .fill(egui::Color32::from_rgb(40, 100, 160))
                                        .corner_radius(3.0),
                                )
                                .clicked()
                            {
                                let from = shorts.capture_from_input.parse::<usize>().unwrap_or(0);
                                let to = shorts.capture_to_input.parse::<usize>().unwrap_or(total);
                                let pictures = std::env::var("USERPROFILE")
                                    .or_else(|_| std::env::var("HOME"))
                                    .map(|h| std::path::PathBuf::from(h).join("Pictures"))
                                    .unwrap_or_else(|_| std::path::PathBuf::from("."));
                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                let dir = pictures.join(format!("xfchess_sequence_{}", ts));
                                replay.current_ply = from;
                                replay.position_dirty = true;
                                replay.paused = true;
                                shorts.capture_mode =
                                    Some(crate::game::shorts_state::CaptureSequence {
                                        from_ply: from,
                                        to_ply: to,
                                        current: from,
                                        delay_secs: replay.speed + 0.15,
                                        timer: 0.0,
                                        output_dir: dir,
                                    });
                                shorts.show_beat_export = false;
                                info!("[SHORTS] Capture sequence started: ply {}–{}", from, to);
                            }
                        }
                    } else {
                        let seq = shorts.capture_mode.as_ref().unwrap();
                        ui.label(
                            egui::RichText::new(format!(
                                "📷 Capturing {}/{}",
                                seq.current, seq.to_ply
                            ))
                            .size(10.0)
                            .color(egui::Color32::from_rgb(100, 200, 255)),
                        );
                    }

                    // Annotation legend in side panel
                    ui.add_space(8.0);
                    ui.add(egui::Separator::default().spacing(4.0));
                    ui.label(
                        egui::RichText::new("Annotations")
                            .size(10.0)
                            .color(egui::Color32::from_gray(120))
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("Right-drag = arrow")
                            .size(9.0)
                            .color(egui::Color32::from_gray(90)),
                    );
                    ui.label(
                        egui::RichText::new("+Shift = orange")
                            .size(9.0)
                            .color(egui::Color32::from_rgb(200, 110, 0)),
                    );
                    ui.label(
                        egui::RichText::new("+Alt = blue")
                            .size(9.0)
                            .color(egui::Color32::from_rgb(80, 140, 220)),
                    );
                    ui.label(
                        egui::RichText::new("Right-click = clear")
                            .size(9.0)
                            .color(egui::Color32::from_gray(90)),
                    );
                });
        });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert an engine `Game` back to a FEN string.
fn engine_to_fen(game: &nimzovich_engine::Game) -> String {
    game_to_fen(game)
}

/// Convert engine piece ID to PieceType.
fn engine_id_to_piece_type(id: i8) -> PieceType {
    use nimzovich_engine::{BISHOP_ID, KING_ID, KNIGHT_ID, PAWN_ID, QUEEN_ID, ROOK_ID};
    match id {
        PAWN_ID => PieceType::Pawn,
        KNIGHT_ID => PieceType::Knight,
        BISHOP_ID => PieceType::Bishop,
        ROOK_ID => PieceType::Rook,
        QUEEN_ID => PieceType::Queen,
        KING_ID => PieceType::King,
        _ => PieceType::Pawn,
    }
}

/// Return the rotation for a piece matching the regular game's `piece_rotation` / `knight_rotation`.
fn replay_piece_rotation(piece_type: PieceType, color: PieceColor) -> Quat {
    match piece_type {
        PieceType::Knight => match color {
            PieceColor::White => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            PieceColor::Black => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        },
        _ => match color {
            PieceColor::White => Quat::IDENTITY,
            PieceColor::Black => Quat::from_rotation_y(std::f32::consts::PI),
        },
    }
}

/// Spawn a single piece for the replay using the same parent+child structure as the regular game.
/// Returns the spawned entity so callers can inject `PieceMoveAnimation` on it.
fn spawn_piece_at_replay(
    commands: &mut Commands,
    meshes: &PieceMeshes,
    material: Handle<StandardMaterial>,
    color: PieceColor,
    piece_type: PieceType,
    position: (u8, u8),
    _visual_offset: Vec3,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) -> Entity {
    let (file, rank) = position;
    let world_pos = Vec3::new(7.0 - file as f32, PIECE_ON_BOARD_Y, rank as f32);

    let mesh = meshes.get(piece_type, color);
    let rotation = replay_piece_rotation(piece_type, color);
    let name = format!("{:?} {:?} at ({},{})", color, piece_type, file, rank);

    commands
        .spawn((
            Piece {
                piece_type,
                color,
                x: file,
                y: rank,
            },
            HasMoved::default(),
            Transform::from_translation(world_pos).with_rotation(rotation),
            Visibility::default(),
            Name::new(name),
            DespawnOnExit(GameState::InGame),
            bevy::camera::visibility::RenderLayers::layer(
                crate::game::systems::camera::BOARD_LAYER,
            ),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::default(),
                Piece3DVisual,
                bevy::camera::visibility::RenderLayers::layer(
                    crate::game::systems::camera::BOARD_LAYER,
                ),
            ));

            if let Some(handles) = sprite_handles {
                let sprite = handles.get(piece_type, color);
                parent.spawn((
                    Sprite::from_image(sprite),
                    Transform::from_xyz(0.0, 0.1, 0.0)
                        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::splat(0.002)),
                    Piece2DVisual,
                    Visibility::Hidden,
                ));
            }
        })
        .id()
}
