//! PGN replay mode (`GameMode::PgnReplay`): step through a parsed game with its own
//! board ([`board`]) and controls ([`ui`]).

use crate::core::{game_scene, GameMode, GameState};
use crate::engine::board_state::ChessEngine;
use crate::game::components::HasMoved;
use crate::game::replay_shorts::ReplayAnnotations;
use crate::game::view_mode::ViewMode;
use crate::motion::{StartPieceMotion, GAMEPLAY_MOVE_SECONDS};
use crate::rendering::pieces::{
    Piece, Piece2DVisual, Piece3DVisual, PieceColor, PieceMeshes, PieceSpriteHandles, PieceType,
    PiecesSpawned, PIECE_ON_BOARD_Y,
};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use xfchess_rules::{game_to_fen, new_game};

mod board;
mod ui;
pub use board::*;
pub use ui::*;

#[derive(Resource, Debug, Clone)]
pub struct ParsedPgnGameResource {
    pub inner: xfchess_rules::ParsedPgnGame,
    pub show_eval_graph: bool,
    pub puzzle_mode: bool,
    pub puzzle_revealed: bool,
}

#[derive(Resource)]
pub struct PgnReplayState {
    /// Owned rules position for the currently selected replay ply.
    pub engine: Box<xfchess_rules::Game>,
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
            engine: Box::new(new_game()),
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

    *replay = PgnReplayState::default();

    replay.fen_snapshots = match pgn.inner.snapshots() {
        Ok(snapshots) => snapshots,
        Err(error) => {
            warn!("[REPLAY] Invalid PGN: {error}");
            return;
        }
    };
    let Some(fen) = replay.fen_snapshots.first() else {
        warn!("[REPLAY] PGN contains no positions");
        return;
    };
    let position = match xfchess_rules::Game::from_fen(fen) {
        Ok(position) => position,
        Err(error) => {
            warn!("[REPLAY] Invalid initial position: {error}");
            return;
        }
    };
    if let Err(error) = engine.set_from_fen(fen) {
        warn!("[REPLAY] Cannot initialize engine: {error}");
        return;
    }
    *replay.engine = position;

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
        if let Ok(position) = xfchess_rules::Game::from_fen(&fen) {
            *replay.engine = position;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_state_does_not_carry_the_engine_inline() {
        // Debug builds copy a resource several times on its way through
        // commands; with the engine inline, leaving a replay overflowed the stack.
        let engine = std::mem::size_of::<xfchess_rules::Game>();
        let state = std::mem::size_of::<PgnReplayState>();
        assert!(engine < 4096, "rules position is {engine} bytes");
        assert!(state < 1024, "replay state is {state} bytes");
    }

    #[test]
    fn forward_backward_seek_pause_and_resume_do_not_publish_moves() -> Result<(), String> {
        use std::time::Duration;
        let pgn = xfchess_rules::parse_pgn("1. e4 e5 2. Nf3 Nc6 *")?;
        let snapshots = pgn.snapshots()?;
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<ChessEngine>();
        app.insert_resource(ParsedPgnGameResource {
            inner: pgn,
            show_eval_graph: false,
            puzzle_mode: false,
            puzzle_revealed: false,
        });
        app.insert_resource(PgnReplayState {
            fen_snapshots: snapshots.clone(),
            ..Default::default()
        });
        app.add_message::<crate::game::events::MoveMadeEvent>();
        app.add_systems(
            Update,
            (
                replay_auto_advance_system,
                replay_apply_move_system,
                replay_sync_engine_system,
            )
                .chain(),
        );
        for (target, animated) in [(1, true), (0, false), (4, false), (0, false)] {
            let mut replay = app.world_mut().resource_mut::<PgnReplayState>();
            replay.current_ply = target;
            replay.position_dirty = true;
            app.update();
            assert_eq!(
                app.world()
                    .resource::<PgnReplayState>()
                    .animate_next_advance,
                animated
            );
            assert_eq!(
                app.world().resource::<ChessEngine>().current_fen(),
                snapshots[target]
            );
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(3));
        app.update();
        assert_eq!(app.world().resource::<PgnReplayState>().current_ply, 0);
        {
            let mut replay = app.world_mut().resource_mut::<PgnReplayState>();
            replay.speed = 0.1;
            replay.timer = Timer::from_seconds(0.1, TimerMode::Once);
            replay.paused = false;
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.11));
        app.update();
        assert_eq!(app.world().resource::<PgnReplayState>().current_ply, 1);
        assert!(app
            .world()
            .resource::<Messages<crate::game::events::MoveMadeEvent>>()
            .is_empty());
        Ok(())
    }
}
