use super::resource::ChessAIResource;
use crate::engine::board_state::ChessEngine;
use crate::game::components::GamePhase;
use crate::game::components::HasMoved;
use crate::game::components::Piece;
use crate::game::resources::{CapturedPieces, CurrentGamePhase, CurrentTurn, MoveHistory};
use crate::game::system_sets::GameSystems;
use crate::game::systems::shared::{execute_move, CapturedTarget, MoveContext};
use bevy::ecs::system::{ParamSet, SystemParam};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};

/// Resource holding the async AI computation task
#[derive(Resource)]
pub struct PendingAIMove(pub Task<Result<AIMove, String>>);

/// AI move representation with Stockfish statistics
#[derive(Debug, Clone)]
pub struct AIMove {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub uci: String,
    pub score: i32,
    pub depth: u8,
    pub thinking_time: f32,
    /// FEN after the move (used to update ChessEngine.fen)
    pub fen_after: String,
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

        // Spawn the Stockfish process at startup (if we can)
        // Wrapped in a task to avoid blocking startup
        let _pool = AsyncComputeTaskPool::get_or_init(Default::default);
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
    pub players: Res<'w, crate::game::resources::player::Players>,
}

/// System params for polling AI task
#[derive(SystemParam)]
pub struct AiPollParams<'w, 's> {
    pub task_resource: Option<ResMut<'w, PendingAIMove>>,
    pub pieces_queries: ParamSet<
        'w,
        's,
        (
            Query<'w, 's, (Entity, &'static mut Piece, &'static mut HasMoved)>,
            Query<'w, 's, (Entity, &'static Piece, &'static HasMoved)>,
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
    pub children_query: Query<'w, 's, &'static Children>,
    pub material_query: Query<'w, 's, &'static MeshMaterial3d<StandardMaterial>>,
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
}

fn spawn_ai_task_system(
    mut commands: Commands,
    mut params: AiSpawnParams,
    braid_manager: Option<Res<crate::multiplayer::braid_node::BraidNodeManager>>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let _start_time = std::time::Instant::now();

    if should_skip_ai_spawn(
        &params.pending_task,
        &params.pending_turn_advance,
        &params.game_phase,
        &params.current_turn,
        &params.ai_config,
        &params.players,
    ) {
        return;
    }

    // Sync ECS → engine FEN so Stockfish sees the latest position
    params
        .engine
        .sync_ecs_to_engine(&params.pieces_query, &params.current_turn);

    let fen = params.engine.current_fen().to_string();
    let depth = params.ai_config.difficulty.stockfish_depth();
    let movetime_ms = params.ai_config.difficulty.stockfish_movetime_ms();
    let _ai_color = params.ai_config.mode.ai_color();

    info!(
        "[AI] Broadcasting board state to Braid network | FEN: {} | depth: {:?} | movetime: {:?}ms",
        fen, depth, movetime_ms
    );

    if let Some(braid_manager) = braid_manager {
        // Trigger Stockfish sidecar via channel
        if let Some(tx) = &braid_manager.sidecar_fen_tx {
            let _ = tx.send(fen.clone());
        }

        // We use a dummy pending move here just to prevent multiple triggers in our singleplayer loops,
        // Braid will resolve the actual move back through `incoming_moves_rx`.
        let task_pool = futures_lite::future::pending();
        let task = AsyncComputeTaskPool::get().spawn(task_pool);
        commands.insert_resource(PendingAIMove(task));
    } else {
        warn!("[AI] BraidNodeManager is unavailable. Stockfish sidecar will not trigger.");
    }
}

/// Helper to check conditions for spawning AI task
fn should_skip_ai_spawn(
    pending_task: &Option<Res<PendingAIMove>>,
    pending_turn_advance: &Option<Res<crate::game::resources::PendingTurnAdvance>>,
    game_phase: &CurrentGamePhase,
    current_turn: &CurrentTurn,
    ai_config: &ChessAIResource,
    players: &crate::game::resources::player::Players,
) -> bool {
    if pending_task.is_some()
        || pending_turn_advance
            .as_ref()
            .map(|r| r.is_pending())
            .unwrap_or(false)
    {
        return true;
    }

    if let crate::game::ai::resource::GameMode::Multiplayer = ai_config.mode {
        return true;
    }

    if game_phase.0 != GamePhase::Playing {
        return true;
    }

    if players.current(current_turn.color).is_human {
        return true;
    }

    if current_turn.color != ai_config.mode.ai_color() {
        return true;
    }

    false
}

/// System that polls the AI task and executes the move when ready
#[allow(clippy::too_many_arguments)]
fn poll_ai_task_system(
    mut commands: Commands,
    mut params: AiPollParams,
    braid_manager: Option<Res<crate::multiplayer::braid_node::BraidNodeManager>>,
) {
    if params.task_resource.is_none() {
        return;
    }

    let mut move_found = None;

    if let Some(braid_manager) = &braid_manager {
        if let Some(rx) = &braid_manager.incoming_moves_rx {
            if let Ok(alg_move) = rx.try_recv() {
                move_found = Some(alg_move);
                commands.remove_resource::<PendingAIMove>();
            }
        }
    }

    if let Some(best_move_str) = move_found {
        if best_move_str.len() >= 4 {
            let from_str = &best_move_str[0..2];
            let to_str = &best_move_str[2..4];
            let promotion_char = best_move_str.chars().nth(4);

            let from_coords = match ChessEngine::uci_to_coords(from_str) {
                Some(c) => c,
                None => return,
            };
            let to_coords = match ChessEngine::uci_to_coords(to_str) {
                Some(c) => c,
                None => return,
            };

            let promotion_type =
                promotion_char.and_then(crate::rendering::pieces::PieceType::from_char);

            info!(
                "[AI] Braid sidecar yielded move: {} -> {} (promotion: {:?})",
                from_str, to_str, promotion_type
            );

            let (move_sound, capture_sound) = if let Some(s) = &params.sounds {
                (Some(s.move_piece.clone()), Some(s.capture_piece.clone()))
            } else {
                (None, None)
            };

            let mut p0 = params.pieces_queries.p0();

            if let Some((entity, piece, is_first_move, capture_target)) =
                find_move_entities(&p0, from_coords, to_coords)
            {
                let ctx = MoveContext {
                    origin: "ai",
                    entity,
                    piece,
                    target: to_coords,
                    capture: capture_target,
                    promotion: promotion_type,
                    was_first_move: is_first_move,
                    remote: false,
                    move_sound,
                    capture_sound,
                };

                execute_move(
                    &ctx,
                    &mut commands,
                    &mut params.pending_turn,
                    &mut params.move_history,
                    &mut params.captured_pieces,
                    &mut params.engine,
                    &mut p0,
                    None,
                    None, // BoardStateSync not available in AI context
                    &params.children_query,
                    &params.material_query,
                    &mut params.materials,
                );
            } else {
                warn!("[AI] Could not find valid piece at {:?}", from_coords);
            }
        }
    }
}

/// Find entity, piece data, and potential capture target for a move
fn find_move_entities(
    pieces_query: &Query<(Entity, &mut Piece, &mut HasMoved)>,
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
