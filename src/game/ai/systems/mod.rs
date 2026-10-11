//! AI move scheduling: decide when the AI should think, run the search on a task,
//! and apply the chosen move. Stockfish process handling is in [`stockfish`].

use super::resource::ChessAIResource;
use crate::core::schedule::GameSystems;
use crate::engine::board_state::ChessEngine;
use crate::game::components::GamePhase;
use crate::game::components::HasMoved;
use crate::game::components::Piece;
use crate::game::events::MoveMadeEvent;
use crate::game::resources::{CapturedPieces, CurrentGamePhase, CurrentTurn, MoveHistory};
use crate::game::systems::shared::{execute_move, CapturedTarget, MoveContext};
use bevy::ecs::system::{ParamSet, SystemParam};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use std::env;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

mod stockfish;
pub use stockfish::*;

#[derive(Resource)]
pub struct PendingAIMove {
    pub task: Task<Result<AIMove, String>>,
    spawned_at: Instant,
    min_reveal_delay: Duration,
}

#[derive(Resource)]
pub struct AIMovePendingReveal {
    result: Result<AIMove, String>,
    ready_at: Instant,
}

#[derive(Resource, Clone)]
pub struct XFChessGamePool(pub std::sync::Arc<std::sync::Mutex<Option<nimzovich_engine::Game>>>);

#[derive(Debug, Clone)]
pub struct AIMove {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub uci: String,
    pub score: i32,
    pub depth: u8,
    pub thinking_time: f32,
    pub nodes: i64,
}

#[derive(Resource, Default, Debug, Reflect)]
#[reflect(Resource)]
pub struct AIStatistics {
    pub last_score: i64,
    pub last_depth: i64,
    pub last_nodes: i64,
    pub thinking_time: f32,
}

pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        use crate::core::states::{GameMode, InGameplay};
        use crate::game::ai::resource::AIEngine;

        // The AI only plays in vs-AI games; replay, spectating, puzzles and
        // online games never start a search.
        app.init_resource::<ChessAIResource>()
            .init_resource::<AIStatistics>()
            .register_type::<ChessAIResource>()
            .register_type::<AIStatistics>()
            .add_systems(
                Update,
                // Think on the frame's final state: after moves, turn flip,
                // check/mate and game-over handling.
                (spawn_ai_task_system, poll_ai_task_system)
                    .chain()
                    .after(crate::game::systems::confirm_game_over_prompt)
                    .in_set(GameSystems::Execution)
                    .run_if(resource_equals(GameMode::SinglePlayer)),
            )
            .add_systems(
                OnEnter(crate::core::GameState::InGame),
                (
                    warmup_xf_engine_pool.run_if(engine_is(AIEngine::XFChessEngine)),
                    warm_up_stockfish.run_if(engine_is(AIEngine::Stockfish)),
                )
                    .run_if(resource_equals(GameMode::SinglePlayer)),
            )
            // A search still running when the player leaves is dropped, not applied
            // to whatever is on the board next.
            .add_systems(OnExit(InGameplay), clear_pending_ai_tasks);

        let _pool = AsyncComputeTaskPool::get_or_init(Default::default);
    }
}

/// Whether `engine` is the chosen AI engine.
pub fn engine_is(
    engine: crate::game::ai::resource::AIEngine,
) -> impl FnMut(Res<ChessAIResource>) -> bool + Clone {
    move |ai: Res<ChessAIResource>| ai.engine == engine
}

pub fn clear_pending_ai_tasks(mut commands: Commands) {
    commands.remove_resource::<PendingAIMove>();
    commands.remove_resource::<AIMovePendingReveal>();
}

fn warmup_xf_engine_pool(mut commands: Commands) {
    let pool_arc = std::sync::Arc::new(std::sync::Mutex::new(None::<nimzovich_engine::Game>));
    let fill = pool_arc.clone();

    AsyncComputeTaskPool::get()
        .spawn(async move {
            let game = nimzovich_engine::game_from_fen(
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            );
            if let Ok(mut slot) = fill.lock() {
                *slot = Some(game);
            }
        })
        .detach();

    commands.insert_resource(XFChessGamePool(pool_arc));
    info!("[AI] XFChess engine warm-up started");
}

#[derive(SystemParam)]
pub struct AiSpawnParams<'w, 's> {
    pub ai_config: Res<'w, ChessAIResource>,
    pub current_turn: Res<'w, CurrentTurn>,
    pub game_phase: Res<'w, CurrentGamePhase>,
    pub pieces_query: Query<'w, 's, (Entity, &'static Piece, &'static HasMoved)>,
    pub pending_task: Option<Res<'w, PendingAIMove>>,
    pub pending_reveal: Option<Res<'w, AIMovePendingReveal>>,
    pub engine: ResMut<'w, ChessEngine>,
    pub pending_turn_advance: Option<Res<'w, crate::game::resources::PendingTurnAdvance>>,
    pub players: Res<'w, crate::game::resources::player::Players>,
    pub sf_process: Option<Res<'w, StockfishProcess>>,
    pub move_history: Res<'w, crate::game::resources::MoveHistory>,
    pub active_tc: Option<Res<'w, crate::game::resources::active_time_control::ActiveTimeControl>>,
    pub game_pool: Option<Res<'w, XFChessGamePool>>,
}

fn compute_think_params(
    base_think: f32,
    half_moves_played: usize,
    active_tc: Option<&crate::game::resources::active_time_control::ActiveTimeControl>,
) -> (f32, Option<u8>) {
    let Some(tc) = active_tc.map(|a| a.control) else {
        return (base_think, None);
    };
    let base_secs = tc.base_seconds();
    let inc_secs = tc.increment_seconds();

    // Reserve clock time so repeated engine searches cannot flag the AI.
    let think_time = if base_secs > 0 {
        let est_moves_left = (40.0 - half_moves_played as f32 / 2.0).max(5.0);
        let increment_bonus = inc_secs as f32 * 0.8;
        let per_move_budget = base_secs as f32 / est_moves_left + increment_bonus;
        base_think
            .min(base_think + inc_secs as f32 * 0.5)
            .min(per_move_budget)
    } else {
        base_think
    };

    // Hard depth cap for fast games with no increment (ultra-bullet / bullet)
    let max_depth = if base_secs > 0 && base_secs < 60 && inc_secs == 0 {
        Some(6u8)
    } else {
        None
    };

    (think_time, max_depth)
}

#[derive(SystemParam)]
pub struct AiPollParams<'w, 's> {
    pub ai_config: Res<'w, ChessAIResource>,
    pub task_resource: Option<ResMut<'w, PendingAIMove>>,
    pub pending_reveal: Option<Res<'w, AIMovePendingReveal>>,
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
    pub engine: ResMut<'w, ChessEngine>,
    pub sounds: Option<Res<'w, crate::game::resources::GameSounds>>,
    pub active_tc: Option<Res<'w, crate::game::resources::active_time_control::ActiveTimeControl>>,
    pub game_pool: Option<Res<'w, XFChessGamePool>>,
    pub move_events: MessageWriter<'w, MoveMadeEvent>,
}

fn spawn_ai_task_system(mut commands: Commands, params: AiSpawnParams) {
    if should_skip_ai_spawn(
        &params.pending_task,
        &params.pending_reveal,
        &params.pending_turn_advance,
        &params.game_phase,
        &params.current_turn,
        &params.ai_config,
        &params.players,
    ) {
        return;
    }

    // FEN is already current — game_logic.rs synced the engine and built the move cache.
    let fen = params.engine.current_fen().to_string();
    let depth = params.ai_config.difficulty.stockfish_depth();
    let movetime_ms = params.ai_config.difficulty.stockfish_movetime_ms();
    let ai_color = params.ai_config.mode.ai_color();

    let spawned_at = Instant::now();
    let (min_delay_ms, max_delay_ms) = params.ai_config.difficulty.thinking_delay_range_ms();
    let min_reveal_delay = Duration::from_millis(rand::random_range(min_delay_ms..=max_delay_ms));

    match params.ai_config.engine {
        crate::game::ai::resource::AIEngine::Stockfish => {
            info!("[AI] Spawning Stockfish task (persistent process)");
            let base_think = movetime_ms.unwrap_or(1500) as f32 / 1000.0;
            let (think_time, max_depth) = compute_think_params(
                base_think,
                params.move_history.len(),
                params.active_tc.as_deref(),
            );
            let depth = max_depth
                .map(|cap| depth.unwrap_or(cap).min(cap))
                .or(depth)
                .unwrap_or(12);
            let movetime = (think_time * 1000.0).round().max(1.0) as u64;
            let strength_elo = params.ai_config.difficulty.stockfish_elo();

            // The task starts the process itself if no warm-up has.
            let process = params.sf_process.as_deref().cloned().unwrap_or_else(|| {
                let process = StockfishProcess::default();
                commands.insert_resource(process.clone());
                process
            });

            let task = spawn_stockfish_task_persistent(fen, depth, movetime, strength_elo, process);
            commands.insert_resource(PendingAIMove {
                task,
                spawned_at,
                min_reveal_delay,
            });
        }
        crate::game::ai::resource::AIEngine::XFChessEngine => {
            let base_think = params.ai_config.difficulty.seconds_per_move();
            let (think_time, max_depth) = compute_think_params(
                base_think,
                params.move_history.len(),
                params.active_tc.as_deref(),
            );
            info!(
                "[AI] Spawning XFChessEngine task — think_time={:.2}s max_depth={:?}",
                think_time, max_depth
            );
            // Reuse the pooled Game to avoid reallocating its 2.2 GB search table each move.
            // Return it to the pool after searching.
            let pool_arc = params.game_pool.as_ref().map(|p| p.0.clone());
            let preloaded = pool_arc.as_ref().and_then(|arc| arc.lock().ok()?.take());
            let task =
                spawn_xf_engine_task(fen, think_time, max_depth, ai_color, preloaded, pool_arc);
            commands.insert_resource(PendingAIMove {
                task,
                spawned_at,
                min_reveal_delay,
            });
        }
    }
}

fn spawn_xf_engine_task(
    fen: String,
    think_time: f32,
    max_depth: Option<u8>,
    color: crate::rendering::pieces::PieceColor,
    preloaded_game: Option<nimzovich_engine::Game>,
    pool: Option<std::sync::Arc<std::sync::Mutex<Option<nimzovich_engine::Game>>>>,
) -> Task<Result<AIMove, String>> {
    AsyncComputeTaskPool::get().spawn(async move {
        let start_time = Instant::now();

        // Reuse the pre-warmed game to skip the 2.2 GB TT zero-write.
        // If the warm-up hasn't finished yet, fall back to allocating a new game.
        let mut game = match preloaded_game {
            Some(mut g) => {
                nimzovich_engine::set_game_from_fen(&mut g, &fen);
                g
            }
            None => nimzovich_engine::game_from_fen(&fen),
        };

        game.secs_per_move = think_time;
        if let Some(d) = max_depth {
            game.abs_max_depth = d as i64;
        }

        let engine_color = match color {
            crate::rendering::pieces::PieceColor::White => 1,
            crate::rendering::pieces::PieceColor::Black => -1,
        };

        let mv = nimzovich_engine::reply(&mut game, engine_color).await;

        let depth_reached = game.max_depth_so_far as u8;

        if let Some(arc) = pool {
            if let Ok(mut guard) = arc.lock() {
                *guard = Some(game);
            }
        }

        let from_file = mv.src as u8 % 8;
        let from_rank = mv.src as u8 / 8;
        let to_file = mv.dst as u8 % 8;
        let to_rank = mv.dst as u8 / 8;

        let from_uci = ChessEngine::coords_to_uci(from_file, from_rank);
        let to_uci = ChessEngine::coords_to_uci(to_file, to_rank);

        let promo_char = match mv.promo {
            5 => "q",
            4 => "r",
            3 => "b",
            2 => "n",
            _ => "",
        };

        let uci = format!("{}{}{}", from_uci, to_uci, promo_char);

        Ok(AIMove {
            from: (from_file, from_rank),
            to: (to_file, to_rank),
            uci,
            score: mv.score as i32,
            depth: depth_reached,
            thinking_time: start_time.elapsed().as_secs_f32(),
            nodes: 0,
        })
    })
}

fn normalized_move_uci(from: (u8, u8), to: (u8, u8), promotion: Option<char>) -> String {
    format!(
        "{}{}{}",
        ChessEngine::coords_to_uci(from.0, from.1),
        ChessEngine::coords_to_uci(to.0, to.1),
        promotion
            .map(|c| c.to_ascii_lowercase())
            .unwrap_or_default()
    )
}

fn should_skip_ai_spawn(
    pending_task: &Option<Res<PendingAIMove>>,
    pending_reveal: &Option<Res<AIMovePendingReveal>>,
    pending_turn_advance: &Option<Res<crate::game::resources::PendingTurnAdvance>>,
    game_phase: &CurrentGamePhase,
    current_turn: &CurrentTurn,
    ai_config: &ChessAIResource,
    players: &crate::game::resources::player::Players,
) -> bool {
    if pending_task.is_some() {
        trace!("[AI] Skipping spawn: pending task already exists");
        return true;
    }

    if pending_reveal.is_some() {
        trace!("[AI] Skipping spawn: move resolved, waiting on reveal delay");
        return true;
    }

    if pending_turn_advance
        .as_ref()
        .map(|r| r.is_pending())
        .unwrap_or(false)
    {
        trace!("[AI] Skipping spawn: turn advance pending");
        return true;
    }

    if let crate::game::ai::resource::AiMode::Multiplayer = ai_config.mode {
        trace!("[AI] Skipping spawn: game mode is Multiplayer");
        return true;
    }

    if !matches!(game_phase.0, GamePhase::Playing | GamePhase::Check) {
        trace!(
            "[AI] Skipping spawn: game phase is {:?}, not Playing/Check",
            game_phase.0
        );
        return true;
    }

    let current_player = players.current(current_turn.color);
    if current_player.is_human {
        trace!(
            "[AI] Skipping spawn: current player ({:?}) is human",
            current_turn.color
        );
        return true;
    }

    let ai_color = ai_config.mode.ai_color();
    if current_turn.color != ai_color {
        trace!(
            "[AI] Skipping spawn: turn color ({:?}) != AI color ({:?})",
            current_turn.color,
            ai_color
        );
        return true;
    }

    info!(
        "[AI] Conditions met - spawning AI task for {:?}",
        current_turn.color
    );
    false
}

#[allow(clippy::too_many_arguments)]
fn poll_ai_task_system(mut commands: Commands, mut params: AiPollParams) {
    // A move already finished computing and is waiting out its reveal delay.
    if let Some(pending_reveal) = params.pending_reveal.as_ref() {
        if Instant::now() < pending_reveal.ready_at {
            return;
        }
        let result = pending_reveal.result.clone();
        commands.remove_resource::<AIMovePendingReveal>();
        apply_ai_result(result, &mut commands, &mut params);
        return;
    }

    let Some(task_resource) = params.task_resource.as_mut() else {
        return;
    };

    // Poll the in-flight search task.
    let Some(result) =
        futures_lite::future::block_on(futures_lite::future::poll_once(&mut task_resource.task))
    else {
        return;
    };

    let ready_at = task_resource.spawned_at + task_resource.min_reveal_delay;
    commands.remove_resource::<PendingAIMove>();

    if Instant::now() >= ready_at {
        apply_ai_result(result, &mut commands, &mut params);
    } else {
        commands.insert_resource(AIMovePendingReveal { result, ready_at });
    }
}

fn apply_ai_result(
    result: Result<AIMove, String>,
    commands: &mut Commands,
    params: &mut AiPollParams,
) {
    let move_found;

    match result {
        Ok(ai_move) => {
            info!(
                "[AI] {:?} search finished: {}",
                params.ai_config.engine, ai_move.uci
            );
            move_found = Some(ai_move.uci);

            params.ai_stats.last_score = ai_move.score as i64;
            params.ai_stats.last_depth = ai_move.depth as i64;
            params.ai_stats.last_nodes = ai_move.nodes;
            params.ai_stats.thinking_time = ai_move.thinking_time;
        }
        Err(e) => {
            error!("[AI] Stockfish task failed: {}", e);
            let base_think = params.ai_config.difficulty.seconds_per_move();
            let (think_time, max_depth) = compute_think_params(
                base_think,
                params.move_history.len(),
                params.active_tc.as_deref(),
            );
            let pool_arc = params.game_pool.as_ref().map(|p| p.0.clone());
            let preloaded = pool_arc.as_ref().and_then(|arc| arc.lock().ok()?.take());
            let fallback = spawn_xf_engine_task(
                params.engine.current_fen().to_string(),
                think_time,
                max_depth,
                params.ai_config.mode.ai_color(),
                preloaded,
                pool_arc,
            );
            commands.insert_resource(PendingAIMove {
                task: fallback,
                spawned_at: Instant::now(),
                min_reveal_delay: Duration::from_millis(0),
            });
            warn!("[AI] Falling back to XFChessEngine for this move");
            return;
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
                "[AI] Playing {} -> {} (promotion: {:?})",
                from_str, to_str, promotion_type
            );

            let (move_sound, capture_sound) = if let Some(s) = &params.sounds {
                (Some(s.move_piece.clone()), Some(s.capture_piece.clone()))
            } else {
                (None, None)
            };

            let mut p0 = params.pieces_queries.p0();

            // Validate with the cached legal-move table — O(1) lookup, no extra generation.
            let move_uci = normalized_move_uci(from_coords, to_coords, promotion_char);

            if !params.engine.is_move_legal_by_uci(&move_uci) {
                warn!("[AI] Engine suggested illegal move {}", move_uci);
                return;
            }

            if let Some((entity, piece, is_first_move, capture_target)) =
                find_move_entities(&p0, from_coords, to_coords)
            {
                debug!("[AI] Found move entities: entity={:?}, piece={:?}, is_first_move={:?}, capture_target={:?}", entity, piece, is_first_move, capture_target);
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
                    game_id: None,
                };

                execute_move(
                    &ctx,
                    commands,
                    &mut params.pending_turn,
                    &mut params.move_history,
                    &mut params.captured_pieces,
                    &mut params.engine,
                    &mut p0,
                    Some(&mut params.move_events),
                    None, // BoardStateSync not available in AI context
                    &params.current_turn,
                );
            } else {
                warn!("[AI] Could not find valid piece at {:?}", from_coords);
            }
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::states::{GameMode, GameState};
    use crate::game::ai::resource::AIEngine;
    use crate::game::resources::player::{Player, Players};
    use crate::rendering::pieces::PieceColor;

    /// A game entered in `mode`, with Stockfish as Black to move.
    fn game_with_ai_to_move(mode: GameMode) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<GameState>()
            .insert_resource(mode)
            .insert_resource(ChessAIResource {
                engine: AIEngine::Stockfish,
                ..Default::default()
            })
            .insert_resource(CurrentTurn {
                color: PieceColor::Black,
                ..Default::default()
            })
            .insert_resource(Players {
                player_1: Player::new(1, "Me".into(), PieceColor::White, true),
                player_2: Player::new(2, "AI".into(), PieceColor::Black, false),
            })
            .init_resource::<CurrentGamePhase>()
            .init_resource::<ChessEngine>()
            .init_resource::<MoveHistory>()
            .init_resource::<CapturedPieces>()
            .init_resource::<crate::game::resources::PendingTurnAdvance>()
            .add_message::<MoveMadeEvent>();
        crate::core::schedule::configure(&mut app);
        app.add_plugins(AIPlugin);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app.update();
        app
    }

    #[test]
    fn the_ai_only_plays_in_vs_ai_games() {
        // Replay once ran the AI here (the previous game's players carried
        // over), looping on moves that were illegal for the replay board.
        for mode in [GameMode::PgnReplay, GameMode::Spectator, GameMode::Puzzle] {
            let app = game_with_ai_to_move(mode);
            let world = app.world();
            assert!(
                !world.contains_resource::<PendingAIMove>(),
                "{mode:?} searched"
            );
            assert!(
                !world.contains_resource::<StockfishProcess>(),
                "{mode:?} started Stockfish"
            );
            assert!(
                !world.contains_resource::<XFChessGamePool>(),
                "{mode:?} warmed the engine"
            );
        }
    }

    #[test]
    fn engine_condition_matches_the_chosen_engine() {
        let mut world = World::new();
        world.insert_resource(ChessAIResource {
            engine: AIEngine::Stockfish,
            ..Default::default()
        });
        use bevy::ecs::system::RunSystemOnce;
        assert!(world
            .run_system_once(engine_is(AIEngine::Stockfish))
            .unwrap());
        assert!(!world
            .run_system_once(engine_is(AIEngine::XFChessEngine))
            .unwrap());
    }

    /// Starts the real engine: `cargo test stockfish_process -- --ignored`.
    #[test]
    #[ignore = "launches the Stockfish binary"]
    fn stockfish_process_starts_once_and_survives_a_failed_search() {
        let _pool = AsyncComputeTaskPool::get_or_init(Default::default);
        if resolve_stockfish_path().is_err() {
            eprintln!("no Stockfish binary; skipping");
            return;
        }
        let process = StockfishProcess::default();
        let search = |fen: &str| {
            futures_lite::future::block_on(spawn_stockfish_task_persistent(
                fen.to_string(),
                8,
                100,
                Some(1600),
                process.clone(),
            ))
        };

        let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let first = search(start).expect("a move from the start position");
        assert!(first.uci.len() >= 4, "{}", first.uci);

        // White is checkmated: no move, so the process is dropped...
        let mated = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
        assert!(search(mated).is_err());
        // ...and the next search starts a fresh one.
        assert!(search(start).is_ok());
    }

    #[test]
    fn limited_stockfish_commands_set_strength() {
        assert_eq!(
            stockfish_strength_commands(Some(1600)),
            vec![
                "setoption name UCI_LimitStrength value true",
                "setoption name UCI_Elo value 1600",
            ]
        );
    }

    #[test]
    fn unrestricted_stockfish_commands_disable_strength_limit() {
        assert_eq!(
            stockfish_strength_commands(None),
            vec!["setoption name UCI_LimitStrength value false"]
        );
    }

    #[test]
    fn search_command_prefers_time_budget() {
        assert_eq!(stockfish_search_command(12, 750), "go movetime 750");
        assert_eq!(stockfish_search_command(12, 0), "go depth 12");
    }

    #[test]
    fn promotion_suffixes_are_preserved_in_uci() {
        let from = ChessEngine::uci_to_coords("e7").unwrap();
        let to = ChessEngine::uci_to_coords("e8").unwrap();
        for promotion in ['q', 'r', 'b', 'n'] {
            assert_eq!(
                normalized_move_uci(from, to, Some(promotion)),
                format!("e7e8{promotion}")
            );
        }
    }
}
