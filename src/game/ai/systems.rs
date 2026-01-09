use super::resource::ChessAIResource;
use crate::game::components::GamePhase;
use crate::game::components::HasMoved;
use crate::game::resources::{
    CapturedPieces, ChessEngine, CurrentGamePhase, CurrentTurn, MoveHistory,
};
use crate::game::system_sets::GameSystems;
use crate::game::systems::shared::{execute_move, CapturedTarget};
use crate::rendering::pieces::{Piece, PieceColor};
use bevy::ecs::system::{ParamSet, SystemParam};
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

/// System params for spawning AI task
#[derive(SystemParam)]
pub struct AiSpawnParams<'w, 's> {
    pub ai_config: Res<'w, ChessAIResource>,
    pub current_turn: Res<'w, CurrentTurn>,
    pub game_phase: Res<'w, CurrentGamePhase>,
    pub pieces_query: Query<'w, 's, (Entity, &'static Piece, &'static HasMoved)>,
    pub pending_task: Option<Res<'w, PendingAIMove>>,
    pub engine: ResMut<'w, ChessEngine>,
    pub pending_turn_advance: Option<Res<'w, crate::game::resources::PendingTurnAdvance>>,
}

/// System params for polling AI task
#[derive(SystemParam)]
pub struct AiPollParams<'w, 's> {
    pub task_resource: Option<ResMut<'w, PendingAIMove>>,
    pub pieces_queries: ParamSet<
        'w,
        's,
        (
            Query<'w, 's, (Entity, &'static mut Piece, &'static mut HasMoved)>, // Mutable access for execution
            Query<'w, 's, (Entity, &'static Piece, &'static HasMoved)>, // Immutable access for scanning
        ),
    >,
    pub current_turn: Res<'w, CurrentTurn>,
    pub move_history: ResMut<'w, MoveHistory>,
    pub captured_pieces: ResMut<'w, CapturedPieces>,
    pub ai_stats: ResMut<'w, AIStatistics>,
    pub pending_turn: ResMut<'w, crate::game::resources::PendingTurnAdvance>,
    pub ai_config: Res<'w, ChessAIResource>,
    pub engine: ResMut<'w, ChessEngine>,
    pub sounds: Option<Res<'w, crate::game::resources::GameSounds>>,
}

/// System that spawns an AI computation task when it's the AI's turn
fn spawn_ai_task_system(mut commands: Commands, mut params: AiSpawnParams) {
    #[cfg(not(target_arch = "wasm32"))]
    let start_time = std::time::Instant::now();

    if should_skip_ai_spawn(
        &params.pending_task,
        &params.pending_turn_advance,
        &params.game_phase,
        &params.current_turn,
        &params.ai_config,
    ) {
        return;
    }

    params
        .engine
        .sync_ecs_to_engine(&params.pieces_query, &params.current_turn);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let sync_duration = start_time.elapsed();
        if sync_duration.as_millis() > 5 {
            warn!(
                "[PERF] sync_ecs_to_engine took {:?}ms",
                sync_duration.as_millis()
            );
        }
    }

    let engine_clone = params.engine.game.clone();
    let think_time = params.ai_config.difficulty.seconds_per_move();
    let ai_color = params.ai_config.mode.ai_color();
    let engine_color: i64 = if ai_color == PieceColor::White { 1 } else { -1 };

    info!("[AI] ========== AI TASK SPAWNED ==========");
    info!(
        "[AI] AI Color: {:?} | Difficulty: {:?} | Think Time: {:.1}s",
        ai_color, params.ai_config.difficulty, think_time
    );
    info!(
        "[AI] Move #{} | Game Phase: {:?}",
        params.current_turn.move_number, params.game_phase.0
    );

    let task_pool = AsyncComputeTaskPool::get();

    // Configurable time limit with sanity cap
    let limited_think_time = think_time.min(5.0);

    let task = task_pool.spawn(async move {
        compute_ai_move_task_body(engine_clone, engine_color, limited_think_time).await
    });

    commands.insert_resource(PendingAIMove(task));

    #[cfg(not(target_arch = "wasm32"))]
    {
        let total_duration = start_time.elapsed();
        if total_duration.as_millis() > 10 {
            warn!(
                "[PERF] spawn_ai_task_system took {:?}ms",
                total_duration.as_millis()
            );
        }
    }
}

/// Helper to check conditions for spawning AI task
fn should_skip_ai_spawn(
    pending_task: &Option<Res<PendingAIMove>>,
    pending_turn_advance: &Option<Res<crate::game::resources::PendingTurnAdvance>>,
    game_phase: &CurrentGamePhase,
    current_turn: &CurrentTurn,
    ai_config: &ChessAIResource,
) -> bool {
    if pending_task.is_some()
        || pending_turn_advance
            .as_ref()
            .map(|r| r.is_pending())
            .unwrap_or(false)
    {
        return true;
    }

    if game_phase.0 != GamePhase::Playing {
        return true;
    }

    if current_turn.color != ai_config.mode.ai_color() {
        return true;
    }

    false
}

/// The computation logic running in the async task
async fn compute_ai_move_task_body(
    mut engine_clone: chess_engine::Game,
    engine_color: i64,
    time_limit: f32,
) -> Result<AIMove, String> {
    // On WASM, we rely on the async yield points in the engine to keep the browser responsive
    // On Native, this runs in a thread pool so it's fine either way

    #[cfg(not(target_arch = "wasm32"))]
    use std::time::Instant;
    #[cfg(target_arch = "wasm32")]
    use web_time::Instant;

    let start = Instant::now();
    engine_clone.secs_per_move = time_limit;

    // We removed catch_unwind because catching panics in async code is complex
    // and the engine should be robust enough.
    let engine_move = reply(&mut engine_clone, engine_color).await;

    let from_x = (engine_move.src % 8) as u8;
    let from_y = (engine_move.src / 8) as u8;
    let to_x = (engine_move.dst % 8) as u8;
    let to_y = (engine_move.dst / 8) as u8;

    let stop_timer = Instant::now();
    let elapsed = stop_timer.duration_since(start).as_secs_f32();

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

/// System that polls the AI task and executes the move when ready
#[allow(clippy::too_many_arguments)]
fn poll_ai_task_system(mut commands: Commands, mut params: AiPollParams) {
    #[cfg(not(target_arch = "wasm32"))]
    let system_start = std::time::Instant::now();

    let Some(mut task_resource) = params.task_resource else {
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

    let (move_sound, capture_sound) = if let Some(s) = &params.sounds {
        (Some(s.move_piece.clone()), Some(s.capture_piece.clone()))
    } else {
        (None, None)
    };

    let mut fallback_needed = false;

    match ai_move_result {
        Ok(ai_move) => {
            info!("[AI] ========== AI MOVE READY FOR EXECUTION ==========");
            params.ai_stats.last_score = ai_move.score;
            params.ai_stats.last_depth = ai_move.depth;
            params.ai_stats.last_nodes = ai_move.nodes_searched;
            params.ai_stats.thinking_time = ai_move.thinking_time;

            let p1_query = params.pieces_queries.p1();
            if let Some((entity, piece, is_first_move, capture_target)) =
                find_move_entities(&p1_query, ai_move.from, ai_move.to)
            {
                // SAFEGUARD: Ensure the AI only moves its own pieces
                // This prevents the bug where AI tries to move player's pieces if engine state desyncs
                if piece.color != params.ai_config.mode.ai_color() {
                    error!(
                        "[AI] CRITICAL ERROR: AI attempted to move opponent's piece! Piece: {:?}, AI Color: {:?}",
                        piece.color,
                        params.ai_config.mode.ai_color()
                    );
                    // Force fallback to find a valid move
                    fallback_needed = true;
                } else {
                    let success = execute_move(
                        "AI",
                        &mut commands,
                        entity,
                        piece,
                        ai_move.to,
                        capture_target,
                        is_first_move,
                        &mut params.pending_turn,
                        &mut params.move_history,
                        &mut params.captured_pieces,
                        &mut params.engine,
                        &mut params.pieces_queries.p0(),
                        move_sound.clone(),
                        capture_sound.clone(),
                    );

                    if !success {
                        error!("[AI] execute_move returned false");
                        fallback_needed = true;
                    }
                }
            } else {
                error!("[AI] Piece not found at {:?}", ai_move.from);
                fallback_needed = true;
            }
        }
        Err(e) => {
            error!("[AI] Engine error: {}", e);
            fallback_needed = true;
        }
    }

    if fallback_needed {
        handle_fallback(
            &mut commands,
            &mut params.engine,
            &params.current_turn,
            &params.ai_config,
            &mut params.pieces_queries,
            &mut params.pending_turn,
            &mut params.move_history,
            &mut params.captured_pieces,
            move_sound,
            capture_sound,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let poll_duration = system_start.elapsed();
        if poll_duration.as_millis() > 5 {
            warn!(
                "[PERF] poll_ai_task_system took {:?}ms",
                poll_duration.as_millis()
            );
        }
    }
}

/// Find entity, piece data, and potential capture target for a move
fn find_move_entities(
    pieces_query: &Query<(Entity, &Piece, &HasMoved)>,
    from: (u8, u8),
    to: (u8, u8),
) -> Option<(Entity, Piece, bool, Option<CapturedTarget>)> {
    let mut move_data = None;
    let mut capture_target = None;

    for (entity, piece, has_moved) in pieces_query.iter() {
        if piece.x == from.0 && piece.y == from.1 {
            move_data = Some((entity, *piece, !has_moved.moved));
        }
        if piece.x == to.0 && piece.y == to.1 {
            capture_target = Some(CapturedTarget {
                entity,
                piece_type: piece.piece_type,
                color: piece.color,
            });
        }
    }

    move_data.map(|(e, p, first)| (e, p, first, capture_target))
}

/// Handle fallback logic when primary AI move fails
fn handle_fallback(
    commands: &mut Commands,
    engine: &mut ResMut<ChessEngine>,
    current_turn: &CurrentTurn,
    ai_config: &ChessAIResource,
    pieces_queries: &mut ParamSet<(
        Query<(Entity, &mut Piece, &mut HasMoved)>,
        Query<(Entity, &Piece, &HasMoved)>,
    )>,
    pending_turn: &mut ResMut<crate::game::resources::PendingTurnAdvance>,
    move_history: &mut ResMut<MoveHistory>,
    captured_pieces: &mut ResMut<CapturedPieces>,
    move_sound: Option<Handle<AudioSource>>,
    capture_sound: Option<Handle<AudioSource>>,
) {
    warn!("[AI] Attempting fallback move...");

    let move_execution_data = {
        let p1 = pieces_queries.p1();
        if let Some(fallback_move) = find_fallback_move(engine, current_turn, ai_config, &p1) {
            find_move_entities(&p1, fallback_move.from, fallback_move.to)
                .map(|(e, p, f, c)| (e, p, f, c, fallback_move.to))
        } else {
            None
        }
    };

    if let Some((entity, piece, is_first_move, capture_target, target_pos)) = move_execution_data {
        execute_move(
            "AI_FALLBACK",
            commands,
            entity,
            piece,
            target_pos,
            capture_target,
            is_first_move,
            pending_turn,
            move_history,
            captured_pieces,
            engine,
            &mut pieces_queries.p0(),
            move_sound,
            capture_sound,
        );
    }
}

/// Find a fallback legal move when the AI engine fails
fn find_fallback_move(
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
