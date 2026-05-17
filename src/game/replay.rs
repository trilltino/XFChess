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
use crate::game::components::HasMoved;
use crate::game::resources::{
    CapturedPieces, CurrentGamePhase, CurrentTurn, GameOverState, GameTimer, MoveHistory,
    PendingTurnAdvance, Selection, TurnStateContext,
};
use crate::game::view_mode::{PlayerViewPreferences, ViewMode};
use crate::rendering::pieces::{
    Piece, PieceColor, PieceMeshes, PieceSpriteHandles, PieceType, PiecesSpawned,
    PIECE_ON_BOARD_Y,
};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use nimzovich_engine::{do_move_with_promo, game_from_fen, game_to_fen, new_game, san_to_move};

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Bevy resource wrapping the parsed PGN game.
#[derive(Resource, Debug, Clone)]
pub struct ParsedPgnGameResource {
    pub inner: nimzovich_engine::ParsedPgnGame,
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
}

impl Default for PgnReplayState {
    fn default() -> Self {
        Self {
            engine: new_game(),
            fen_snapshots: vec!["rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()],
            current_ply: 0,
            paused: true,
            speed: 1.0,
            timer: Timer::from_seconds(1.0, TimerMode::Once),
            board_ready: false,
            position_dirty: false,
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

    info!("[REPLAY] Setting up replay: {} moves", pgn.inner.moves.len());

    // Reset replay state
    *replay = PgnReplayState::default();
    replay.engine = new_game();

    // Pre-generate all FEN snapshots by applying moves sequentially
    let mut temp_engine = new_game();
    replay.fen_snapshots.clear();
    replay.fen_snapshots.push("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string());

    for (i, san) in pgn.inner.moves.iter().enumerate() {
        match san_to_move(&mut temp_engine, san) {
            Ok((src, dst, promo)) => {
                do_move_with_promo(&mut temp_engine, src, dst, true, promo);
                let fen = engine_to_fen(&temp_engine);
                replay.fen_snapshots.push(fen);
            }
            Err(e) => {
                warn!("[REPLAY] Failed to resolve move {} '{}': {:?}", i + 1, san, e);
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

    // If we have a FEN snapshot, rebuild from it (handles both forward and backward)
    if target_ply < replay.fen_snapshots.len() {
        let fen = replay.fen_snapshots[target_ply].clone();
        replay.engine = game_from_fen(&fen);
    } else {
        // Shouldn't happen if snapshots were generated correctly
        warn!("[REPLAY] Missing FEN snapshot for ply {}", target_ply);
    }
}

/// Sync the replay engine to the main ChessEngine resource so the board
/// rendering (2D and 3D) sees the correct position.
pub fn replay_sync_engine_system(
    replay: Res<PgnReplayState>,
    mut engine: ResMut<ChessEngine>,
) {
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

    // Spawn pieces from engine board
    for sq in 0..64 {
        let piece_id = replay.engine.board[sq];
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

        let base_color = if color == PieceColor::White {
            Color::WHITE
        } else {
            Color::BLACK
        };
        let piece_material = materials.add(StandardMaterial {
            base_color,
            ..default()
        });

        spawn_piece_at_replay(
            &mut commands,
            &piece_meshes,
            piece_material,
            color,
            piece_type,
            (file, rank),
            Vec3::ZERO,
            &sprite_handles,
        );
    }

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
    parsed_pgn: Option<Res<ParsedPgnGameResource>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut view_prefs: ResMut<PlayerViewPreferences>,
    game_mode: Res<GameMode>,
) {
    if *game_mode != GameMode::PgnReplay {
        return;
    }

    let Some(pgn) = parsed_pgn else { return };
    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

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
                }
                if btn(ui, "<").clicked() {
                    if replay.current_ply > 0 {
                        replay.current_ply -= 1;
                        replay.position_dirty = true;
                    }
                    replay.paused = true;
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
                    if replay.current_ply < pgn.inner.moves.len() {
                        replay.current_ply += 1;
                        replay.position_dirty = true;
                    }
                    replay.paused = true;
                }
                if btn(ui, ">>|").clicked() {
                    replay.current_ply = pgn.inner.moves.len();
                    replay.position_dirty = true;
                    replay.paused = true;
                }

                ui.add_space(12.0);

                // Speed slider
                ui.label(egui::RichText::new("Speed:").size(12.0).color(egui::Color32::LIGHT_GRAY));
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
                let view_label = match view_prefs.local_view {
                    ViewMode::Standard2D => "3D",
                    ViewMode::Standard3D => "2D",
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
                    view_prefs.toggle_view();
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
                });
            });
        });

    // --- Move list panel (right side) ---
    let total = pgn.inner.moves.len();
    if total == 0 {
        return;
    }

    egui::SidePanel::right("replay_move_list")
        .max_width(200.0)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(25, 25, 25, 220),
            inner_margin: egui::Margin::symmetric(8, 8),
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.heading(
                egui::RichText::new("Moves")
                    .size(14.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.add_space(6.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for move_num in 1..=((total + 1) / 2) {
                        let white_idx = (move_num - 1) * 2;
                        let black_idx = white_idx + 1;

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{}.", move_num))
                                    .size(11.0)
                                    .color(egui::Color32::GRAY),
                            );

                            // White move
                            if white_idx < total {
                                let is_current = replay.current_ply == white_idx + 1;
                                let text = &pgn.inner.moves[white_idx];
                                let color = if is_current {
                                    egui::Color32::from_rgb(100, 200, 255)
                                } else {
                                    egui::Color32::WHITE
                                };
                                let resp = ui.selectable_label(is_current, egui::RichText::new(text).size(11.0).color(color));
                                if resp.clicked() {
                                    replay.current_ply = white_idx + 1;
                                    replay.position_dirty = true;
                                    replay.paused = true;
                                }
                            }

                            // Black move
                            if black_idx < total {
                                let is_current = replay.current_ply == black_idx + 1;
                                let text = &pgn.inner.moves[black_idx];
                                let color = if is_current {
                                    egui::Color32::from_rgb(100, 200, 255)
                                } else {
                                    egui::Color32::WHITE
                                };
                                let resp = ui.selectable_label(is_current, egui::RichText::new(text).size(11.0).color(color));
                                if resp.clicked() {
                                    replay.current_ply = black_idx + 1;
                                    replay.position_dirty = true;
                                    replay.paused = true;
                                }
                            }
                        });
                    }
                });

            // Show current ply info
            ui.add_space(8.0);
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "Ply {}/{}",
                    replay.current_ply,
                    total
                ))
                .size(11.0)
                .color(egui::Color32::LIGHT_GRAY),
            );

            // Tags
            if let Some(white) = pgn.inner.tag("White") {
                ui.label(egui::RichText::new(format!("White: {}", white)).size(10.0).color(egui::Color32::LIGHT_GRAY));
            }
            if let Some(black) = pgn.inner.tag("Black") {
                ui.label(egui::RichText::new(format!("Black: {}", black)).size(10.0).color(egui::Color32::LIGHT_GRAY));
            }
            if !pgn.inner.result.is_empty() {
                ui.label(egui::RichText::new(format!("Result: {}", pgn.inner.result)).size(10.0).color(egui::Color32::LIGHT_GRAY));
            }
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

/// Spawn a single piece for the replay (simplified version of spawn_piece_at).
fn spawn_piece_at_replay(
    commands: &mut Commands,
    meshes: &PieceMeshes,
    material: Handle<StandardMaterial>,
    color: PieceColor,
    piece_type: PieceType,
    position: (u8, u8),
    _visual_offset: Vec3,
    sprite_handles: &Option<Res<PieceSpriteHandles>>,
) {
    let (file, rank) = position;
    let world_pos = Vec3::new(file as f32, PIECE_ON_BOARD_Y, rank as f32);

    let mesh = meshes.get(piece_type, color);
    let name = format!("{:?} {:?} at ({},{})", color, piece_type, file, rank);

    // Determine if this is the piece's first move based on position
    let has_moved = match (piece_type, color, file, rank) {
        // Pawns: not on starting rank = has moved
        (PieceType::Pawn, PieceColor::White, _, r) if r != 1 => true,
        (PieceType::Pawn, PieceColor::Black, _, r) if r != 6 => true,
        // Rooks: not on corners = has moved
        (PieceType::Rook, PieceColor::White, 0, 0) => false,
        (PieceType::Rook, PieceColor::White, 7, 0) => false,
        (PieceType::Rook, PieceColor::Black, 0, 7) => false,
        (PieceType::Rook, PieceColor::Black, 7, 7) => false,
        // Kings: not on e-file starting rank = has moved
        (PieceType::King, PieceColor::White, 4, 0) => false,
        (PieceType::King, PieceColor::Black, 4, 7) => false,
        // Knights: on b/g file starting rank = not moved
        (PieceType::Knight, PieceColor::White, f, 0) if f == 1 || f == 6 => false,
        (PieceType::Knight, PieceColor::Black, f, 7) if f == 1 || f == 6 => false,
        // Bishops: on c/f file starting rank = not moved
        (PieceType::Bishop, PieceColor::White, f, 0) if f == 2 || f == 5 => false,
        (PieceType::Bishop, PieceColor::Black, f, 7) if f == 2 || f == 5 => false,
        // Queens: on d-file starting rank = not moved
        (PieceType::Queen, PieceColor::White, 3, 0) => false,
        (PieceType::Queen, PieceColor::Black, 3, 7) => false,
        _ => true,
    };

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(world_pos),
        Piece {
            piece_type,
            color,
            x: file,
            y: rank,
        },
        HasMoved { moved: has_moved, move_count: 0 },
        Name::new(name),
        DespawnOnExit(GameState::InGame),
    ));

    // 2D sprite visual (if handles available)
    if let Some(handles) = sprite_handles {
        let sprite = handles.get(piece_type, color);
        commands.spawn((
            Sprite {
                image: sprite,
                custom_size: Some(Vec2::splat(0.8)),
                ..default()
            },
            Transform::from_translation(world_pos + Vec3::Y * 0.1),
            Piece {
                piece_type,
                color,
                x: file,
                y: rank,
            },
            Name::new(format!("2D {:?} {:?}", color, piece_type)),
            DespawnOnExit(GameState::InGame),
        ));
    }
}
