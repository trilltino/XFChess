use crate::core::{DespawnOnExit, GameMode, GameState};
use crate::engine::board_state::ChessEngine;
use crate::game::components::{HasMoved, PieceMoveAnimation};
use crate::game::replay_shorts::ReplayAnnotations;
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


#[derive(Resource, Debug, Clone)]
pub struct ParsedPgnGameResource {
    pub inner: nimzovich_engine::ParsedPgnGame,
    pub show_eval_graph: bool,
    pub puzzle_mode: bool,
    pub puzzle_revealed: bool,
}

#[derive(Resource)]
pub struct PgnReplayState {
    pub engine: nimzovich_engine::Game,
    pub fen_snapshots: Vec<String>,
    pub current_ply: usize,
    pub paused: bool,
    pub speed: f32,
    pub timer: Timer,
    pub board_ready: bool,
    pub position_dirty: bool,
    pub show_controls: bool,

    // ── Cinematic / shorts ──
    pub prev_board: [i8; 64],
    pub engine_ply: usize,
    pub animate_next_advance: bool,
    pub slow_factor: f32,
    pub cinematic_timer: f32,
    pub last_annotation_ply: usize,

    // ── In-replayer PGN file picker ──
    pub pgn_load_rx: Option<crossbeam_channel::Receiver<Result<String, String>>>,
    pub is_loading_file: bool,
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
            show_controls: true,
            prev_board: [0i8; 64],
            engine_ply: 0,
            animate_next_advance: false,
            slow_factor: 1.0,
            cinematic_timer: 0.0,
            last_annotation_ply: usize::MAX,
            pgn_load_rx: None,
            is_loading_file: false,
            pgn_input_error: None,
        }
    }
}

impl PgnReplayState {
    pub fn total_plies(&self) -> usize {
        self.fen_snapshots.len().saturating_sub(1)
    }
}


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

pub fn cleanup_replay(
    mut commands: Commands,
    pieces: Query<Entity, With<Piece>>,
    mut replay: ResMut<PgnReplayState>,
) {
    for entity in pieces.iter() {
        commands.entity(entity).despawn();
    }
    *replay = PgnReplayState::default();
    commands.remove_resource::<ParsedPgnGameResource>();
    info!("[REPLAY] Cleaned up replay resources");
}


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

pub fn replay_sync_engine_system(replay: Res<PgnReplayState>, mut engine: ResMut<ChessEngine>) {
    let fen = engine_to_fen(&replay.engine);
    if engine.fen != fen {
        engine.set_from_fen(&fen).ok();
    }
}


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


pub fn replay_ui_system(
    mut contexts: EguiContexts,
    mut replay: ResMut<PgnReplayState>,
    mut parsed_pgn: Option<ResMut<ParsedPgnGameResource>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut view_mode: ResMut<ViewMode>,
    game_mode: Res<GameMode>,
    eval_history: Option<Res<crate::ui::game::game_2d::EvalHistory>>,
    mut annotations: ResMut<ReplayAnnotations>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    if *game_mode != GameMode::PgnReplay {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if keyboard.just_pressed(KeyCode::KeyH) {
        replay.show_controls = !replay.show_controls;
    }

    if replay.pgn_load_rx.is_some() {
        let rx = replay.pgn_load_rx.take().unwrap();
        match rx.try_recv() {
            Ok(Ok(text)) => {
                replay.is_loading_file = false;
                match nimzovich_engine::parse_pgn(&text) {
                    Ok(pgn) => {
                        let mut temp = new_game_no_tt();
                        let mut snapshots =
                            vec!["rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
                                .to_string()];
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
                        commands.insert_resource(ParsedPgnGameResource {
                            inner: pgn,
                            show_eval_graph: false,
                            puzzle_mode: false,
                            puzzle_revealed: false,
                        });
                        // Resource inserted; it will be visible next frame.
                        return;
                    }
                    Err(e) => {
                        replay.pgn_input_error = Some(format!("{:?}", e));
                    }
                }
            }
            Ok(Err(e)) => {
                replay.is_loading_file = false;
                replay.pgn_input_error = Some(e);
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {
                replay.pgn_load_rx = Some(rx);
            }
            Err(_) => {
                replay.is_loading_file = false;
                replay.pgn_input_error = Some("File loading was interrupted.".to_string());
            }
        }
    }

    // No PGN loaded yet — show the file picker overlay centred on screen.
    if parsed_pgn.is_none() {
        egui::Window::new("pgn_load_overlay")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::Vec2::new(480.0, 280.0))
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
                    ui.label(
                        egui::RichText::new("PGN Replay")
                            .size(22.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Exit").clicked() {
                            replay.pgn_load_rx = None;
                            replay.is_loading_file = false;
                            replay.pgn_input_error = None;
                            commands.insert_resource(PgnReplayState::default());
                            commands.remove_resource::<ParsedPgnGameResource>();
                            next_state.set(GameState::MainMenu);
                        }
                    });
                });
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Choose a .pgn file to replay and analyze.")
                        .size(13.0)
                        .color(egui::Color32::from_rgb(160, 170, 190)),
                );
                ui.add_space(8.0);

                #[cfg(not(target_os = "android"))]
                {
                    if ui
                        .add_enabled(
                            !replay.is_loading_file,
                            egui::Button::new(
                                egui::RichText::new("Choose .pgn File")
                                    .size(15.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(50, 120, 60))
                            .corner_radius(5.0)
                            .min_size(egui::Vec2::new(180.0, 38.0)),
                        )
                        .clicked()
                    {
                        replay.is_loading_file = true;
                        let (tx, rx) = crossbeam_channel::bounded(1);
                        replay.pgn_load_rx = Some(rx);
                        std::thread::spawn(move || {
                            let result = (|| {
                                let path = rfd::FileDialog::new()
                                    .add_filter("PGN Files", &["pgn"])
                                    .set_title("Choose a PGN file")
                                    .pick_file()
                                    .ok_or("No file selected")?;
                                std::fs::read_to_string(path).map_err(|e| e.to_string())
                            })();
                            let _ = tx.send(result);
                        });
                    }
                }

                #[cfg(target_os = "android")]
                {
                    ui.label(
                        egui::RichText::new("File picker is not available on Android.")
                            .size(13.0)
                            .color(egui::Color32::from_rgb(160, 170, 190)),
                    );
                }

                if let Some(ref err) = replay.pgn_input_error.clone() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("Error: {}", err))
                            .size(10.5)
                            .color(egui::Color32::from_rgb(230, 100, 80)),
                    );
                }
            });
        return;
    }

    let Some(ref pgn) = parsed_pgn else { return };

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

    if replay.show_controls {
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
                            [52.0, 34.0],
                            egui::Button::new(egui::RichText::new(label).size(17.0).strong())
                                .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
                                .corner_radius(5.0),
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
                                replay.current_ply += 1;
                                replay.position_dirty = true;
                            }
                        }
                        replay.paused = true;
                        annotations.arrows.clear();
                        annotations.highlights.clear();
                        annotations.dirty = true;
                    }
                    if btn(ui, ">>|").clicked() {
                        if let Some(ref pgn_res) = parsed_pgn {
                            replay.current_ply = pgn_res.inner.moves.len();
                            replay.position_dirty = true;
                        }
                        replay.paused = true;
                        annotations.arrows.clear();
                        annotations.highlights.clear();
                        annotations.dirty = true;
                    }

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
                            [60.0, 34.0],
                            egui::Button::new(egui::RichText::new(view_label).size(14.0).strong())
                                .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
                                .corner_radius(5.0),
                        )
                        .clicked()
                    {
                        view_mode.toggle();
                        annotations.dirty = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized(
                                [120.0, 34.0],
                                egui::Button::new(
                                    egui::RichText::new("Exit to Menu").size(13.0).strong(),
                                )
                                .fill(egui::Color32::from_rgb(120, 70, 70))
                                .corner_radius(5.0),
                            )
                            .clicked()
                        {
                            replay.pgn_load_rx = None;
                            replay.is_loading_file = false;
                            replay.pgn_input_error = None;
                            commands.insert_resource(PgnReplayState::default());
                            commands.remove_resource::<ParsedPgnGameResource>();
                            next_state.set(GameState::MainMenu);
                        }

                        if ui
                            .add_sized(
                                [100.0, 30.0],
                                egui::Button::new(
                                    egui::RichText::new("Hide H").size(12.0).strong(),
                                )
                                .fill(egui::Color32::from_rgba_unmultiplied(55, 55, 55, 200))
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            replay.show_controls = false;
                        }
                    });
                });
            });
    }

    let Some(pgn) = parsed_pgn else { return };
    let total = pgn.inner.moves.len();
    if total == 0 {
        return;
    }
    // Show the full PGN move list without puzzle gating.
    let visible_total = total;

    egui::SidePanel::right("replay_move_list")
        .min_width(300.0)
        .max_width(380.0)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(18, 18, 24, 230),
            inner_margin: egui::Margin::symmetric(12, 10),
            ..Default::default()
        })
        .show(ctx, |ui| {
            // PGN header (Lichess-style: White / Black / Result)
            if let Some(white) = pgn.inner.tag("White") {
                ui.label(
                    egui::RichText::new(format!("♔ {}", white))
                        .size(14.0)
                        .color(egui::Color32::from_gray(220)),
                );
            }
            if let Some(black) = pgn.inner.tag("Black") {
                ui.label(
                    egui::RichText::new(format!("♚ {}", black))
                        .size(14.0)
                        .color(egui::Color32::from_gray(160)),
                );
            }
            if !pgn.inner.result.is_empty() {
                ui.label(
                    egui::RichText::new(&pgn.inner.result)
                        .size(15.0)
                        .color(egui::Color32::GOLD)
                        .strong(),
                );
            }
            ui.add(egui::Separator::default().spacing(8.0));

            // Move list — Lichess 3-column grid: index | white | black
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    egui::Grid::new("replay_move_grid")
                        .num_columns(3)
                        .min_col_width(30.0)
                        .spacing([4.0, 3.0])
                        .show(ui, |ui| {
                            for move_num in 1..=((visible_total + 1) / 2) {
                                let white_idx = (move_num - 1) * 2;
                                let black_idx = white_idx + 1;

                                // Index column
                                ui.label(
                                    egui::RichText::new(format!("{}.", move_num))
                                        .size(13.0)
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
                                            .size(14.0)
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
                                            .size(14.0)
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
                                            .size(14.0)
                                            .color(egui::Color32::DARK_GRAY),
                                    );
                                } else {
                                    ui.label("");
                                }

                                ui.end_row();
                            }
                        });

                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new(format!("Ply {}/{}", replay.current_ply, total))
                            .size(12.0)
                            .color(egui::Color32::DARK_GRAY),
                    );

                });
        });
}


fn engine_to_fen(game: &nimzovich_engine::Game) -> String {
    game_to_fen(game)
}

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
