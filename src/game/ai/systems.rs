//! AI systems for move computation and execution
//!
//! This module implements the AI opponent logic using the full chess engine
//! with alpha-beta pruning, transposition tables, and iterative deepening.
//!
//! **Bevy 0.17 AsyncComputeTaskPool**: Uses Bevy's task pool for non-blocking AI computation
//! instead of manual thread spawning. This integrates better with Bevy's ECS and prevents
//! frame drops during AI thinking.

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future;
use crate::core::GameState;
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::game::components::{GamePhase, HasMoved};
use crate::game::resources::{CurrentTurn, CurrentGamePhase, Selection, MoveHistory, GameTimer, CapturedPieces};
use crate::game::components::MoveRecord;
use crate::game::system_sets::GameSystems;
use super::resource::{ChessAIResource, GameMode};
use chess_engine::{new_game, reply, Game as EngineGame, Move as EngineMove};

/// Resource holding the async AI computation task
#[derive(Resource)]
pub struct PendingAIMove(pub Task<AIMove>);

/// AI move representation with engine statistics
#[derive(Debug, Clone)]
pub struct AIMove {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub score: i64,
    pub depth: i64,
    pub nodes_searched: i64,
}

/// Resource to track AI statistics
#[derive(Resource, Default, Debug, Reflect)]
#[reflect(Resource)]
pub struct AIStatistics {
    pub last_score: i64,
    pub last_depth: i64,
    pub last_nodes: i64,
    pub thinking_time: f32,
}

/// Plugin for AI systems
pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<ChessAIResource>()
            .init_resource::<AIStatistics>()
            .register_type::<ChessAIResource>()
            .register_type::<AIStatistics>()
            .add_systems(
                Update,
                (
                    spawn_ai_task_system,
                    poll_ai_task_system,
                )
                .chain()
                .in_set(GameSystems::Execution),
            );
    }
}

/// Synchronize ECS board to chess engine format
///
/// Copies all pieces and critical game state from the ECS world to the chess engine.
/// This includes piece positions, castling rights, and move counter.
fn sync_ecs_to_engine(
    pieces_with_moved: &Query<(Entity, &Piece, &HasMoved)>,
    current_turn: &CurrentTurn,
    engine: &mut EngineGame
) {
    // Clear the board
    engine.board = [0; 64];

    let mut piece_count = 0;
    let mut white_pieces = 0;
    let mut black_pieces = 0;

    // Copy all pieces to engine board AND synchronize castling rights in one pass
    for (_, piece, has_moved) in pieces_with_moved.iter() {
        let square = (piece.y * 8 + piece.x) as usize;
        piece_count += 1;

        // Copy piece to board
        let piece_id = match piece.piece_type {
            PieceType::Pawn => 1,
            PieceType::Knight => 2,
            PieceType::Bishop => 3,
            PieceType::Rook => 4,
            PieceType::Queen => 5,
            PieceType::King => 6,
        };

        engine.board[square] = if piece.color == PieceColor::White {
            white_pieces += 1;
            piece_id
        } else {
            black_pieces += 1;
            -piece_id
        };

        // Synchronize castling rights
        if piece.piece_type == PieceType::King {
            if piece.color == PieceColor::White {
                engine.white_king_has_moved = has_moved.moved;
            } else {
                engine.black_king_has_moved = has_moved.moved;
            }
        } else if piece.piece_type == PieceType::Rook {
            // Check starting positions for rooks
            match (piece.color, square) {
                (PieceColor::White, 0) => engine.white_rook_0_has_moved = has_moved.moved,
                (PieceColor::White, 7) => engine.white_rook_7_has_moved = has_moved.moved,
                (PieceColor::Black, 56) => engine.black_rook_56_has_moved = has_moved.moved,
                (PieceColor::Black, 63) => engine.black_rook_63_has_moved = has_moved.moved,
                _ => {} // Rook not in starting position, doesn't affect castling
            }
        }
    }

    // Synchronize move counter
    // Chess engines use 0-indexed move counter, so subtract 1
    engine.move_counter = (current_turn.move_number - 1) as i32;

    // Board sync debug (only in verbose mode to avoid spam)
    // info!("[AI] Board sync: {} pieces (W:{} B:{})", piece_count, white_pieces, black_pieces);
}

/// System that spawns an AI computation task when it's the AI's turn
fn spawn_ai_task_system(
    mut commands: Commands,
    ai_config: Res<ChessAIResource>,
    current_turn: Res<CurrentTurn>,
    game_phase: Res<CurrentGamePhase>,
    mut pieces_query: Query<(Entity, &Piece, &HasMoved)>,
    pending_task: Option<Res<PendingAIMove>>,
) {
    // Don't spawn if:
    // - A task is already running
    // - Game is not in Playing phase
    // - Mode is not VsAI
    // - It's not the AI's turn
    if pending_task.is_some() {
        return;
    }

    if game_phase.0 != GamePhase::Playing {
        return;
    }

    let GameMode::VsAI { ai_color } = ai_config.mode else {
        return;
    };

    if current_turn.color != ai_color {
        return;
    }

    // Initialize chess engine
    let mut engine = new_game();

    // Sync ECS board to engine (including castling rights and move counter)
    sync_ecs_to_engine(
        &pieces_query,
        &current_turn,
        &mut engine
    );

    // Configure engine
    let think_time = ai_config.difficulty.seconds_per_move();
    engine.secs_per_move = think_time;

    // Determine color for engine (1 = White, -1 = Black)
    let engine_color = if ai_color == PieceColor::White { 1 } else { -1 };

    info!("[AI] ========== AI TASK SPAWNED ==========");
    info!("[AI] AI Color: {:?} | Difficulty: {:?} | Think Time: {:.1}s",
        ai_color, ai_config.difficulty, think_time);
    info!("[AI] Move #{} | Game Phase: {:?}", current_turn.move_number, game_phase.0);

    // Spawn async task on Bevy's compute task pool (non-blocking)
    let task_pool = AsyncComputeTaskPool::get();
    let task = task_pool.spawn(async move {
        use std::time::Instant;
        let start = Instant::now();

        // Run chess engine to find best move for the correct color
        let engine_move: EngineMove = reply(&mut engine, engine_color);

        let elapsed = start.elapsed().as_secs_f32();

        // Convert engine move to our format
        let from_x = (engine_move.src % 8) as u8;
        let from_y = (engine_move.src / 8) as u8;
        let to_x = (engine_move.dst % 8) as u8;
        let to_y = (engine_move.dst / 8) as u8;

        info!("[AI] ========== AI COMPUTATION COMPLETE ==========");
        info!("[AI] Best Move: ({},{}) -> ({},{})", from_x, from_y, to_x, to_y);
        info!("[AI] Evaluation: Score={} | Depth={} | Nodes={} | Time={:.2}s",
            engine_move.score, engine.max_depth_so_far, engine.calls, elapsed);

        let nps = if elapsed > 0.0 { engine.calls as f32 / elapsed } else { 0.0 };
        info!("[AI] Performance: {:.0} nodes/sec | Avg depth: {:.1}",
            nps, engine.max_depth_so_far as f32);

        AIMove {
            from: (from_x, from_y),
            to: (to_x, to_y),
            score: engine_move.score,
            depth: engine.max_depth_so_far,
            nodes_searched: engine.calls,
        }
    });

    commands.insert_resource(PendingAIMove(task));
}

/// System that polls the AI task and executes the move when ready
#[allow(clippy::too_many_arguments)]
fn poll_ai_task_system(
    mut commands: Commands,
    task_resource: Option<ResMut<PendingAIMove>>,
    mut pieces_query: Query<(Entity, &mut Piece, &mut HasMoved, &mut Transform)>,
    mut current_turn: ResMut<CurrentTurn>,
    mut move_history: ResMut<MoveHistory>,
    mut game_timer: ResMut<GameTimer>,
    mut captured_pieces: ResMut<CapturedPieces>,
    mut ai_stats: ResMut<AIStatistics>,
) {
    // Early return if no AI task is running
    let Some(mut task_resource) = task_resource else {
        return;
    };

    // Poll the async task to check if it's complete
    if let Some(ai_move) = future::block_on(future::poll_once(&mut task_resource.0)) {
        info!("[AI] ========== AI MOVE READY FOR EXECUTION ==========");
        info!("[AI] Move: ({},{}) -> ({},{}) | Score: {} | Depth: {} | Nodes: {}",
              ai_move.from.0, ai_move.from.1, ai_move.to.0, ai_move.to.1,
              ai_move.score, ai_move.depth, ai_move.nodes_searched);

        // Update AI statistics
        let score_delta = ai_move.score - ai_stats.last_score;
        ai_stats.last_score = ai_move.score;
        ai_stats.last_depth = ai_move.depth;
        ai_stats.last_nodes = ai_move.nodes_searched;

        if ai_stats.last_score != 0 {
            info!("[AI] Score Change: {} (previous: {})",
                if score_delta > 0 { format!("+{}", score_delta) } else { score_delta.to_string() },
                ai_stats.last_score - score_delta);
        }

        // Remove the task resource
        commands.remove_resource::<PendingAIMove>();

        // Find the piece to move
        let mut moving_piece_entity = None;
        let mut moving_piece_data = None;

        for (entity, piece, _, _) in pieces_query.iter() {
            if piece.x == ai_move.from.0 && piece.y == ai_move.from.1 {
                moving_piece_entity = Some(entity);
                moving_piece_data = Some(*piece);
                break;
            }
        }

        if let (Some(entity), Some(_piece_data)) = (moving_piece_entity, moving_piece_data) {
            // Check for capture
            let mut captured_entity = None;
            let mut captured_piece_type = None;
            let mut captured_piece_color = None;

            for (other_entity, other_piece, _, _) in pieces_query.iter() {
                if other_piece.x == ai_move.to.0 && other_piece.y == ai_move.to.1 {
                    captured_entity = Some(other_entity);
                    captured_piece_type = Some(other_piece.piece_type);
                    captured_piece_color = Some(other_piece.color);
                    break;
                }
            }

            // Despawn captured piece if any and track it
            if let Some(captured) = captured_entity {
                commands.entity(captured).despawn();

                // Track the capture
                if let (Some(piece_type), Some(piece_color)) = (captured_piece_type, captured_piece_color) {
                    captured_pieces.add_capture(piece_color, piece_type);
                    info!("[AI] CAPTURE! AI took {:?} {:?} at ({}, {})",
                        piece_color, piece_type, ai_move.to.0, ai_move.to.1);

                    let advantage = captured_pieces.material_advantage();
                    info!("[AI] Material Advantage: {}",
                        if advantage > 0 {
                            format!("White +{} pawns", advantage)
                        } else if advantage < 0 {
                            format!("Black +{} pawns", -advantage)
                        } else {
                            "Equal".to_string()
                        });
                }
            }

            // Execute the move
            if let Ok((_, mut piece, mut has_moved, _)) = pieces_query.get_mut(entity) {
                // Record the move
                let move_record = MoveRecord {
                    piece_type: piece.piece_type,
                    piece_color: piece.color,
                    from: (piece.x, piece.y),
                    to: ai_move.to,
                    captured: captured_piece_type,
                    is_castling: false,
                    is_en_passant: false,
                    is_check: false,
                    is_checkmate: false,
                };
                move_history.add_move(move_record);

                // Update piece position
                let from_pos = (piece.x, piece.y);
                piece.x = ai_move.to.0;
                piece.y = ai_move.to.1;

                // Mark as moved
                let was_first_move = !has_moved.moved;
                has_moved.moved = true;
                has_moved.move_count += 1;

                info!("[AI] Move Executed: {:?} {:?}: ({},{}) -> ({},{}){}",
                    piece.color, piece.piece_type,
                    from_pos.0, from_pos.1, ai_move.to.0, ai_move.to.1,
                    if was_first_move { " [FIRST MOVE]" } else { "" });

                // Apply Fischer increment
                let time_before = match piece.color {
                    PieceColor::White => game_timer.white_time_left,
                    PieceColor::Black => game_timer.black_time_left,
                };
                game_timer.apply_increment(piece.color);
                let time_after = match piece.color {
                    PieceColor::White => game_timer.white_time_left,
                    PieceColor::Black => game_timer.black_time_left,
                };
                info!("[AI] Timer: +{:.1}s increment ({:.1}s -> {:.1}s)",
                    time_after - time_before, time_before, time_after);

                // Switch turns
                current_turn.switch();
                info!("[AI] Turn Switch: Now {:?}'s turn | Move #{}",
                    current_turn.color, current_turn.move_number);
                info!("[AI] Time Remaining: White {:.1}s | Black {:.1}s",
                    game_timer.white_time_left, game_timer.black_time_left);
            }
        }
    }
}
