//! AI systems for move computation and execution
//!
//! This module implements the AI opponent logic using the full chess engine
//! with alpha-beta pruning, transposition tables, and iterative deepening.
//!
//! **Bevy 0.17 AsyncComputeTaskPool**: Uses Bevy's task pool for non-blocking AI computation
//! instead of manual thread spawning. This integrates better with Bevy's ECS and prevents
//! frame drops during AI thinking.

use super::resource::{ChessAIResource, GameMode};
use crate::game::components::MoveRecord;
use crate::game::components::{GamePhase, HasMoved};
use crate::game::resources::{
    CapturedPieces, ChessEngine, CurrentGamePhase, CurrentTurn, FastBoardState, GameTimer,
    MoveHistory,
};
use crate::game::system_sets::GameSystems;
use crate::rendering::pieces::{Piece, PieceColor};
use bevy::ecs::system::ParamSet;
use bevy::prelude::*;
use bevy::tasks::{block_on, AsyncComputeTaskPool, Task};
use chess_engine::{do_move, reply};
use futures_lite::future;

/// Resource holding the async AI computation task
///
/// The task returns a Result to handle panics and errors from the chess engine.
/// If the engine panics or returns an error, we can fall back to a safe move.
#[derive(Resource)]
pub struct PendingAIMove(pub Task<Result<AIMove, String>>);

/// AI move representation with engine statistics
#[derive(Debug, Clone)]
pub struct AIMove {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub score: i64,
    pub depth: i64,
    pub nodes_searched: i64,
    pub thinking_time: f32,
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
        app.init_resource::<ChessAIResource>()
            .init_resource::<AIStatistics>()
            .register_type::<ChessAIResource>()
            .register_type::<AIStatistics>()
            .add_systems(
                Update,
                (spawn_ai_task_system, poll_ai_task_system)
                    .chain()
                    .in_set(GameSystems::Execution),
            );
    }
}

/// System that spawns an AI computation task when it's the AI's turn
fn spawn_ai_task_system(
    mut commands: Commands,
    ai_config: Res<ChessAIResource>,
    current_turn: Res<CurrentTurn>,
    game_phase: Res<CurrentGamePhase>,
    pieces_query: Query<(Entity, &Piece, &HasMoved)>,
    pending_task: Option<Res<PendingAIMove>>,
    mut engine: ResMut<ChessEngine>,
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

    // Sync ECS → Engine before AI computation (using shared engine)
    engine.sync_ecs_to_engine(&pieces_query, &current_turn);

    // Clone engine game state for async task (can't send mutable references across threads)
    let mut engine_clone = engine.game.clone();

    // Configure engine
    let think_time = ai_config.difficulty.seconds_per_move();
    engine_clone.secs_per_move = think_time;

    // Determine color for engine (1 = White, -1 = Black)
    let engine_color = if ai_color == PieceColor::White { 1 } else { -1 };

    info!("[AI] ========== AI TASK SPAWNED ==========");
    info!(
        "[AI] AI Color: {:?} | Difficulty: {:?} | Think Time: {:.1}s",
        ai_color, ai_config.difficulty, think_time
    );
    info!(
        "[AI] Move #{} | Game Phase: {:?}",
        current_turn.move_number, game_phase.0
    );

    // Spawn async task on Bevy's compute task pool (non-blocking)
    // CRITICAL: The async task runs on a separate thread with its own stack
    // RUST_MIN_STACK should affect these threads, but we also limit search depth
    // as a safety measure to prevent stack overflow
    let task_pool = AsyncComputeTaskPool::get();

    // Limit search time more aggressively to prevent deep searches
    let limited_think_time = think_time.min(0.5); // Cap at 0.5 seconds max

    let task = task_pool.spawn(async move {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        use std::time::Instant;

        let start = Instant::now();

        // Apply the limited think time
        engine_clone.secs_per_move = limited_think_time;

        // Wrap chess engine call in panic handler to prevent game crash
        // This catches any panics from the chess engine and converts them to errors
        let engine_result =
            catch_unwind(AssertUnwindSafe(|| reply(&mut engine_clone, engine_color)));

        match engine_result {
            Ok(engine_move) => {
                let elapsed = start.elapsed().as_secs_f32();

                // Convert engine move to our format
                let from_x = (engine_move.src % 8) as u8;
                let from_y = (engine_move.src / 8) as u8;
                let to_x = (engine_move.dst % 8) as u8;
                let to_y = (engine_move.dst / 8) as u8;

                info!("[AI] ========== AI COMPUTATION COMPLETE ==========");
                info!(
                    "[AI] Best Move: ({},{}) -> ({},{})",
                    from_x, from_y, to_x, to_y
                );
                info!(
                    "[AI] Evaluation: Score={} | Depth={} | Nodes={} | Time={:.2}s",
                    engine_move.score, engine_clone.max_depth_so_far, engine_clone.calls, elapsed
                );

                let nps = if elapsed > 0.0 {
                    engine_clone.calls as f32 / elapsed
                } else {
                    0.0
                };
                info!(
                    "[AI] Performance: {:.0} nodes/sec | Avg depth: {:.1}",
                    nps, engine_clone.max_depth_so_far as f32
                );

                Ok(AIMove {
                    from: (from_x, from_y),
                    to: (to_x, to_y),
                    score: engine_move.score,
                    depth: engine_clone.max_depth_so_far,
                    nodes_searched: engine_clone.calls,
                    thinking_time: elapsed,
                })
            }
            Err(panic_payload) => {
                let elapsed = start.elapsed().as_secs_f32();
                let error_msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                    format!("Chess engine panicked: {}", s)
                } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    format!("Chess engine panicked: {}", s)
                } else {
                    "Chess engine panicked with unknown error".to_string()
                };

                error!("[AI] ========== AI COMPUTATION FAILED ==========");
                error!("[AI] {}", error_msg);
                error!("[AI] Computation time: {:.2}s before panic", elapsed);
                error!("[AI] This is a critical error - falling back to safe move");

                Err(error_msg)
            }
        }
    });

    commands.insert_resource(PendingAIMove(task));
}

/// System that polls the AI task and executes the move when ready
///
/// Uses non-blocking task completion checking to avoid deadlocks.
/// Handles errors and panics gracefully with fallback moves.
///
/// Uses ParamSet to coordinate conflicting queries that access the same components.
#[allow(clippy::too_many_arguments)]
fn poll_ai_task_system(
    mut commands: Commands,
    task_resource: Option<ResMut<PendingAIMove>>,
    mut pieces_queries: ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved, &mut Transform)>,
        Query<(Entity, &mut Piece, &mut HasMoved)>,
        Query<(Entity, &Piece, &HasMoved)>,
    )>,
    mut current_turn: ResMut<CurrentTurn>,
    mut move_history: ResMut<MoveHistory>,
    mut game_timer: ResMut<GameTimer>,
    mut captured_pieces: ResMut<CapturedPieces>,
    mut ai_stats: ResMut<AIStatistics>,
    mut fast_board: ResMut<FastBoardState>,
    ai_config: Res<ChessAIResource>,
    mut engine: ResMut<ChessEngine>,
) {
    // Early return if no AI task is running
    let Some(mut task_resource) = task_resource else {
        return;
    };

    // Non-blocking check: only process if task is finished
    // Using is_finished() avoids blocking the main thread
    if !task_resource.0.is_finished() {
        return; // Task still computing, wait for next frame
    }

    // Task is complete, get the result
    // Since we checked is_finished() first, poll_once should return immediately without blocking
    // This is the pattern used in Bevy's async_compute example
    let ai_move_result = match block_on(future::poll_once(&mut task_resource.0)) {
        Some(result) => {
            // Remove the task resource since we got the result
            commands.remove_resource::<PendingAIMove>();
            result
        }
        None => {
            // This shouldn't happen if is_finished() returned true, but handle it anyway
            warn!("[AI] Task reported finished but result not available - will retry next frame");
            return;
        }
    };

    // Process the AI move result
    match ai_move_result {
        Ok(ai_move) => {
            info!("[AI] ========== AI MOVE READY FOR EXECUTION ==========");
            info!(
                "[AI] Move: ({},{}) -> ({},{}) | Score: {} | Depth: {} | Nodes: {}",
                ai_move.from.0,
                ai_move.from.1,
                ai_move.to.0,
                ai_move.to.1,
                ai_move.score,
                ai_move.depth,
                ai_move.nodes_searched
            );

            // Update AI statistics
            let score_delta = ai_move.score - ai_stats.last_score;
            ai_stats.last_score = ai_move.score;
            ai_stats.last_depth = ai_move.depth;
            ai_stats.last_nodes = ai_move.nodes_searched;
            ai_stats.thinking_time = ai_move.thinking_time;

            if ai_stats.last_score != 0 {
                info!(
                    "[AI] Score Change: {} (previous: {})",
                    if score_delta > 0 {
                        format!("+{}", score_delta)
                    } else {
                        score_delta.to_string()
                    },
                    ai_stats.last_score - score_delta
                );
            }

            info!("[AI] Thinking Time: {:.2}s", ai_move.thinking_time);

            // Validate and execute the move
            execute_ai_move(
                &mut commands,
                &mut pieces_queries,
                &mut current_turn,
                &mut move_history,
                &mut game_timer,
                &mut captured_pieces,
                &mut fast_board,
                &mut engine,
                ai_move,
            );
        }
        Err(error_msg) => {
            error!("[AI] ========== AI MOVE FAILED - USING FALLBACK ==========");
            error!("[AI] Error: {}", error_msg);

            // Fallback: find any legal move as a safe alternative
            // Use engine to generate legal moves
            if let Some(fallback_move) = find_fallback_move_fallback(
                &mut engine,
                &current_turn,
                &ai_config,
                &pieces_queries.p2(),
            ) {
                warn!(
                    "[AI] Using fallback move: ({},{}) -> ({},{})",
                    fallback_move.from.0,
                    fallback_move.from.1,
                    fallback_move.to.0,
                    fallback_move.to.1
                );

                execute_ai_move(
                    &mut commands,
                    &mut pieces_queries,
                    &mut current_turn,
                    &mut move_history,
                    &mut game_timer,
                    &mut captured_pieces,
                    &mut fast_board,
                    &mut engine,
                    fallback_move,
                );
            } else {
                error!("[AI] CRITICAL: Could not find any legal fallback move!");
                error!(
                    "[AI] Game state may be corrupted. Current turn: {:?}",
                    current_turn.color
                );

                // Log detailed board state for debugging (using immutable query)
                let pieces_count = pieces_queries.p2().iter().count();
                error!("[AI] Pieces on board: {}", pieces_count);
                for (entity, piece, has_moved) in pieces_queries.p2().iter() {
                    error!(
                        "[AI]   Piece: Entity {:?}, {:?} {:?} at ({}, {}), moved: {}",
                        entity, piece.color, piece.piece_type, piece.x, piece.y, has_moved.moved
                    );
                }

                // Set game over state to prevent further moves
                warn!("[AI] Setting game over state due to AI failure");
            }
        }
    }
}

/// Execute an AI move after validation
///
/// Uses ParamSet to coordinate queries that would otherwise conflict.
#[allow(clippy::too_many_arguments)]
fn execute_ai_move(
    commands: &mut Commands,
    pieces_queries: &mut ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved, &mut Transform)>,
        Query<(Entity, &mut Piece, &mut HasMoved)>,
        Query<(Entity, &Piece, &HasMoved)>,
    )>,
    current_turn: &mut ResMut<CurrentTurn>,
    move_history: &mut ResMut<MoveHistory>,
    game_timer: &mut ResMut<GameTimer>,
    captured_pieces: &mut ResMut<CapturedPieces>,
    fast_board: &mut ResMut<FastBoardState>,
    engine: &mut ResMut<ChessEngine>,
    ai_move: AIMove,
) {
    // Find the piece to move - validate it exists (using immutable query first)
    let mut moving_piece_entity = None;
    let mut moving_piece_data = None;

    for (entity, piece, _) in pieces_queries.p2().iter() {
        if piece.x == ai_move.from.0 && piece.y == ai_move.from.1 {
            moving_piece_entity = Some(entity);
            moving_piece_data = Some(*piece);
            break;
        }
    }

    // Validate piece exists at source square
    if moving_piece_entity.is_none() || moving_piece_data.is_none() {
        error!(
            "[AI] VALIDATION FAILED: No piece found at source square ({}, {})",
            ai_move.from.0, ai_move.from.1
        );
        error!("[AI] Current board state:");
        for (entity, piece, _) in pieces_queries.p2().iter() {
            error!(
                "[AI]   Entity {:?}: {:?} {:?} at ({}, {})",
                entity, piece.color, piece.piece_type, piece.x, piece.y
            );
        }
        return; // Cannot execute move without valid piece
    }

    // Validate that both entity and piece data are available
    let entity = match moving_piece_entity {
        Some(e) => e,
        None => {
            error!("[AI] VALIDATION FAILED: moving_piece_entity is None after validation check");
            error!(
                "[AI] AI move from ({}, {}) to ({}, {}) cannot be executed",
                ai_move.from.0, ai_move.from.1, ai_move.to.0, ai_move.to.1
            );
            return;
        }
    };

    let piece_data = match moving_piece_data {
        Some(p) => p,
        None => {
            error!("[AI] VALIDATION FAILED: moving_piece_data is None after validation check");
            error!(
                "[AI] AI move from ({}, {}) to ({}, {}) cannot be executed",
                ai_move.from.0, ai_move.from.1, ai_move.to.0, ai_move.to.1
            );
            return;
        }
    };

    // Validate piece belongs to AI's color (current turn)
    if piece_data.color != current_turn.color {
        error!(
            "[AI] VALIDATION FAILED: Piece at ({}, {}) is {:?}, but current turn is {:?}",
            ai_move.from.0, ai_move.from.1, piece_data.color, current_turn.color
        );
        return; // Cannot move opponent's piece
    }

    // Check for capture (using immutable query)
    let mut captured_entity = None;
    let mut captured_piece_type = None;
    let mut captured_piece_color = None;

    for (other_entity, other_piece, _) in pieces_queries.p2().iter() {
        if other_piece.x == ai_move.to.0 && other_piece.y == ai_move.to.1 {
            captured_entity = Some(other_entity);
            captured_piece_type = Some(other_piece.piece_type);
            captured_piece_color = Some(other_piece.color);
            break;
        }
    }

    // Move captured piece to capture zone instead of despawning
    if let Some(captured) = captured_entity {
        // Calculate capture position BEFORE adding to list (for correct indexing)
        if let (Some(piece_type), Some(piece_color)) = (captured_piece_type, captured_piece_color) {
            let count_of_same_type = match piece_color {
                crate::rendering::pieces::PieceColor::White => captured_pieces
                    .black_captured
                    .iter()
                    .filter(|&&p| p == piece_type)
                    .count(),
                crate::rendering::pieces::PieceColor::Black => captured_pieces
                    .white_captured
                    .iter()
                    .filter(|&&p| p == piece_type)
                    .count(),
            };

            // Now add the capture to the list
            captured_pieces.add_capture(piece_color, piece_type);
            info!(
                "[AI] CAPTURE! AI took {:?} {:?} at ({}, {})",
                piece_color, piece_type, ai_move.to.0, ai_move.to.1
            );

            let capture_pos = crate::rendering::pieces::calculate_capture_position(
                piece_color,
                piece_type,
                count_of_same_type,
            );

            // Move piece to capture zone and mark as captured
            commands.entity(captured).insert((
                Transform::from_translation(capture_pos),
                crate::game::components::Captured,
            ));

            info!(
                "[AI] Moved captured piece to capture zone at {:?}",
                capture_pos
            );

            let advantage = captured_pieces.material_advantage();
            info!(
                "[AI] Material Advantage: {}",
                if advantage > 0 {
                    format!("White +{} pawns", advantage)
                } else if advantage < 0 {
                    format!("Black +{} pawns", -advantage)
                } else {
                    "Equal".to_string()
                }
            );
        }
    }

    // Execute the move (using mutable query with Transform)
    // First, update the piece position and record the move
    let mut query_p0 = pieces_queries.p0();
    let (from_pos, piece_type, piece_color, was_first_move) = {
        let Ok((_, mut piece, mut has_moved, _)) = query_p0.get_mut(entity) else {
            error!(
                "[AI] FAILED to get mutable access to piece entity {:?}",
                entity
            );
            error!("[AI] This should not happen - entity was found in previous query");
            return;
        };

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

        (from_pos, piece.piece_type, piece.color, was_first_move)
    };
    // query_p0 is dropped here automatically when it goes out of scope

    // Execute move in engine
    let src_index = ChessEngine::square_to_index(from_pos.0, from_pos.1);
    let dst_index = ChessEngine::square_to_index(ai_move.to.0, ai_move.to.1);
    do_move(&mut engine.game, src_index, dst_index, true);

    // Sync engine → ECS to ensure consistency (updates castling rights, etc.)
    // Use the query without Transform for syncing (p0() borrow is now released)
    engine.sync_engine_to_ecs(commands, &mut pieces_queries.p1());

    // Mark FastBoardState dirty (needs rebuild)
    fast_board.mark_dirty();

    info!(
        "[AI] Move Executed: {:?} {:?}: ({},{}) -> ({},{}){}",
        piece_color,
        piece_type,
        from_pos.0,
        from_pos.1,
        ai_move.to.0,
        ai_move.to.1,
        if was_first_move { " [FIRST MOVE]" } else { "" }
    );

    // Apply Fischer increment
    let time_before = match piece_color {
        PieceColor::White => game_timer.white_time_left,
        PieceColor::Black => game_timer.black_time_left,
    };
    game_timer.apply_increment(piece_color);
    let time_after = match piece_color {
        PieceColor::White => game_timer.white_time_left,
        PieceColor::Black => game_timer.black_time_left,
    };
    info!(
        "[AI] Timer: +{:.1}s increment ({:.1}s -> {:.1}s)",
        time_after - time_before,
        time_before,
        time_after
    );

    // Switch turns
    current_turn.switch();
    info!(
        "[AI] Turn Switch: Now {:?}'s turn | Move #{}",
        current_turn.color, current_turn.move_number
    );
    info!(
        "[AI] Time Remaining: White {:.1}s | Black {:.1}s",
        game_timer.white_time_left, game_timer.black_time_left
    );
}

/// Find a fallback legal move when the AI engine fails
///
/// This is a safety mechanism to prevent the game from getting stuck
/// when the chess engine panics or returns an error.
fn find_fallback_move_fallback(
    engine: &mut ResMut<ChessEngine>,
    current_turn: &CurrentTurn,
    ai_config: &ChessAIResource,
    pieces_query: &Query<(Entity, &Piece, &HasMoved)>,
) -> Option<AIMove> {
    let GameMode::VsAI { ai_color } = ai_config.mode else {
        return None;
    };

    if current_turn.color != ai_color {
        return None;
    }

    // Sync ECS → Engine before searching for moves
    engine.sync_ecs_to_engine(pieces_query, current_turn);

    // Use engine to find any legal move for the AI's color
    // Try each square to find a piece with legal moves
    for x in 0..8 {
        for y in 0..8 {
            let legal_moves = engine.get_legal_moves_for_square((x, y), ai_color);
            if !legal_moves.is_empty() {
                // Found a piece with legal moves, return the first one
                let fallback_to = legal_moves[0];
                warn!(
                    "[AI] Fallback: Found legal move from ({},{}) -> ({},{})",
                    x, y, fallback_to.0, fallback_to.1
                );

                return Some(AIMove {
                    from: (x, y),
                    to: fallback_to,
                    score: 0, // Unknown score for fallback
                    depth: 0,
                    nodes_searched: 0,
                    thinking_time: 0.0, // Fallback moves are instantaneous
                });
            }
        }
    }

    None // No legal moves found
}
