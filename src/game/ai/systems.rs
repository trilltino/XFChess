use super::resource::ChessAIResource;
use crate::game::components::GamePhase;
use crate::game::components::HasMoved;
use crate::game::resources::{
    CapturedPieces, ChessEngine, CurrentGamePhase, CurrentTurn, MoveHistory,
};
use crate::game::system_sets::GameSystems;
use crate::game::systems::shared::{execute_move, CapturedTarget};
use crate::rendering::pieces::{Piece, PieceColor};
use bevy::ecs::system::ParamSet;
use bevy::prelude::*;
use bevy::tasks::{block_on, AsyncComputeTaskPool, Task};
use chess_engine::reply;
use futures_lite::future;

/// Resource holding the async AI computation task
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
    pending_turn_advance: Option<Res<crate::game::resources::PendingTurnAdvance>>,
) {
    if pending_task.is_some()
        || pending_turn_advance
            .map(|r| r.is_pending())
            .unwrap_or(false)
    {
        return;
    }

    if game_phase.0 != GamePhase::Playing {
        return;
    }

    let ai_color = ai_config.mode.ai_color();

    if current_turn.color != ai_color {
        return;
    }

    engine.sync_ecs_to_engine(&pieces_query, &current_turn);
    let mut engine_clone = engine.game.clone();
    let think_time = ai_config.difficulty.seconds_per_move();
    engine_clone.secs_per_move = think_time;
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

    let task_pool = AsyncComputeTaskPool::get();

    // Configurable time limit (removed hard 0.5s cap, trusting config)
    // But keeping a sanity cap to prevent freezing on extremely high settings
    let limited_think_time = think_time.min(5.0);

    let task = task_pool.spawn(async move {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        use std::time::Instant;

        let start = Instant::now();
        engine_clone.secs_per_move = limited_think_time;
        let engine_result =
            catch_unwind(AssertUnwindSafe(|| reply(&mut engine_clone, engine_color)));

        match engine_result {
            Ok(engine_move) => {
                let elapsed = start.elapsed().as_secs_f32();
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

                Err(error_msg)
            }
        }
    });

    commands.insert_resource(PendingAIMove(task));
}

/// System that polls the AI task and executes the move when ready
#[allow(clippy::too_many_arguments)]
fn poll_ai_task_system(
    mut commands: Commands,
    task_resource: Option<ResMut<PendingAIMove>>,
    mut pieces_queries: ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>, // Mutable access for execution
        Query<(Entity, &Piece, &HasMoved)>,         // Immutable access for scanning
    )>,
    current_turn: Res<CurrentTurn>,
    mut move_history: ResMut<MoveHistory>,

    mut captured_pieces: ResMut<CapturedPieces>,
    mut ai_stats: ResMut<AIStatistics>,
    mut pending_turn: ResMut<crate::game::resources::PendingTurnAdvance>,
    ai_config: Res<ChessAIResource>,
    mut engine: ResMut<ChessEngine>,
    sounds: Option<Res<crate::game::resources::GameSounds>>,
) {
    let Some(mut task_resource) = task_resource else {
        return;
    };

    if !task_resource.0.is_finished() {
        return;
    }

    let ai_move_result = match block_on(future::poll_once(&mut task_resource.0)) {
        Some(result) => {
            commands.remove_resource::<PendingAIMove>();
            result
        }
        None => {
            warn!("[AI] Task reported finished but result not available");
            return;
        }
    };

    let p1_query = pieces_queries.p1(); // Immutable borrow
    let fallback_needed = match ai_move_result {
        Ok(ai_move) => {
            info!("[AI] ========== AI MOVE READY FOR EXECUTION ==========");

            // Stats update
            ai_stats.last_score = ai_move.score;
            ai_stats.last_depth = ai_move.depth;
            ai_stats.last_nodes = ai_move.nodes_searched;
            ai_stats.thinking_time = ai_move.thinking_time;

            // Find entity and prepare move
            let mut move_data = None;
            for (entity, piece, has_moved) in p1_query.iter() {
                if piece.x == ai_move.from.0 && piece.y == ai_move.from.1 {
                    move_data = Some((entity, *piece, !has_moved.moved));
                    break;
                }
            }

            // Check capture (using p1 again)
            let mut capture_target = None;
            for (entity, piece, _) in p1_query.iter() {
                if piece.x == ai_move.to.0 && piece.y == ai_move.to.1 {
                    capture_target = Some(CapturedTarget {
                        entity,
                        piece_type: piece.piece_type,
                        color: piece.color,
                    });
                    break;
                }
            }

            if let Some((entity, piece, was_first_move)) = move_data {
                // Drop immutable borrow not needed for Copy type

                // Get sound handles
                let (move_sound, capture_sound) = if let Some(s) = &sounds {
                    (Some(s.move_piece.clone()), Some(s.capture_piece.clone()))
                } else {
                    (None, None)
                };

                // Execute move using shared logic
                let success = execute_move(
                    "AI",
                    &mut commands,
                    entity,
                    piece,
                    ai_move.to,
                    capture_target,
                    was_first_move,
                    &mut pending_turn,
                    &mut move_history,
                    &mut captured_pieces,
                    &mut engine,
                    &mut pieces_queries.p0(),
                    move_sound,
                    capture_sound,
                );

                if !success {
                    error!("[AI] execute_move returned false");
                    true // Fallback
                } else {
                    false // Success
                }
            } else {
                error!("[AI] Piece not found at {:?}", ai_move.from);
                true // Fallback
            }
        }
        Err(e) => {
            error!("[AI] Engine error: {}", e);
            true
        }
    };

    if fallback_needed {
        // Fallback logic
        warn!("[AI] Attempting fallback move...");
        let p1_query = pieces_queries.p1();
        if let Some(fallback_move) =
            find_fallback_move_fallback(&mut engine, &current_turn, &ai_config, &p1_query)
        {
            // drop(p1_query); // Redundant

            // Execute fallback... (simplified: reuse logic above? duplicate limited fallback code here for safety)
            // We need to find entity again for fallback move
            // This is getting recursive.
            // Simplest: just run engine one ply?
            // Since this is rare, I'll basically copy the execution block or extract it.
            // But I already extracted `execute_move`.

            // Let's implement fallback execution inline quickly
            let (move_sound, capture_sound) = if let Some(s) = &sounds {
                (Some(s.move_piece.clone()), Some(s.capture_piece.clone()))
            } else {
                (None, None)
            };

            let p1 = pieces_queries.p1();
            let mut move_data = None;
            let mut capture_target = None;

            for (entity, piece, has_moved) in p1.iter() {
                if piece.x == fallback_move.from.0 && piece.y == fallback_move.from.1 {
                    move_data = Some((entity, *piece, !has_moved.moved));
                }
                if piece.x == fallback_move.to.0 && piece.y == fallback_move.to.1 {
                    capture_target = Some(CapturedTarget {
                        entity,
                        piece_type: piece.piece_type,
                        color: piece.color,
                    });
                }
            }
            // drop(p1); // Redundant

            if let Some((entity, piece, was_first_move)) = move_data {
                execute_move(
                    "AI_FALLBACK",
                    &mut commands,
                    entity,
                    piece,
                    fallback_move.to,
                    capture_target,
                    was_first_move,
                    &mut pending_turn,
                    &mut move_history,
                    &mut captured_pieces,
                    &mut engine,
                    &mut pieces_queries.p0(),
                    move_sound,
                    capture_sound,
                );
            }
        }
    }
}

/// Find a fallback legal move when the AI engine fails
fn find_fallback_move_fallback(
    engine: &mut ResMut<ChessEngine>,
    current_turn: &CurrentTurn,
    ai_config: &ChessAIResource,
    pieces_query: &Query<(Entity, &Piece, &HasMoved)>,
) -> Option<AIMove> {
    let ai_color = ai_config.mode.ai_color();

    if current_turn.color != ai_color {
        return None;
    }

    engine.sync_ecs_to_engine(pieces_query, current_turn);

    for x in 0..8 {
        for y in 0..8 {
            let legal_moves = engine.get_legal_moves_for_square((x, y), ai_color);
            if !legal_moves.is_empty() {
                let fallback_to = legal_moves[0];
                return Some(AIMove {
                    from: (x, y),
                    to: fallback_to,
                    score: 0,
                    depth: 0,
                    nodes_searched: 0,
                    thinking_time: 0.0,
                });
            }
        }
    }

    None
}
