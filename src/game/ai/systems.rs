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
use std::env;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

/// Resource holding the async AI computation task
#[derive(Resource)]
pub struct PendingAIMove(pub Task<Result<AIMove, String>>);

/// Persistent, pre-warmed nimzovich engine game.
///
/// Avoids the 2.2 GB TT zero-write that `game_from_fen` / `new_game` triggers on
/// every AI call.  The async task takes the game out (`Option` becomes `None`),
/// calls `set_game_from_fen` (reuses the existing TT allocation), runs the search,
/// then puts the game back.  A background warm-up task fills the pool on game entry
/// so it is ready before the player's first move.
#[derive(Resource, Clone)]
pub struct XFChessGamePool(pub std::sync::Arc<std::sync::Mutex<Option<nimzovich_engine::Game>>>);

struct StockfishInner {
    stdin: std::process::ChildStdin,
    reader: std::io::BufReader<std::process::ChildStdout>,
    initialized: bool,
}

impl StockfishInner {
    fn new() -> Result<Self, String> {
        let stockfish_path = resolve_stockfish_path()?;
        let mut child = Command::new(&stockfish_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                format!(
                    "Failed to spawn Stockfish at '{}': {}",
                    stockfish_path.display(),
                    e
                )
            })?;

        let stdin = child.stdin.take().ok_or("Failed to get Stockfish stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to get Stockfish stdout")?;
        let reader = std::io::BufReader::new(stdout);
        drop(child);
        Ok(Self {
            stdin,
            reader,
            initialized: false,
        })
    }
}

/// Persistent Stockfish process, shared via Arc<Mutex<>> so async tasks can reuse it.
#[derive(Resource, Clone)]
pub struct StockfishProcess(std::sync::Arc<std::sync::Mutex<StockfishInner>>);

/// AI move representation with Stockfish statistics
#[derive(Debug, Clone)]
pub struct AIMove {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub uci: String,
    pub score: i32,
    pub depth: u8,
    pub thinking_time: f32,
}

fn resolve_stockfish_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("STOCKFISH_PATH") {
        let override_path = PathBuf::from(path.trim());
        if override_path.exists() {
            return Ok(override_path);
        }

        return Err(format!(
            "STOCKFISH_PATH is set but the file does not exist: {}",
            override_path.display()
        ));
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("stockfish.exe"));
        candidates.push(current_dir.join("stockfish"));
        candidates.push(current_dir.join("assets").join("bin").join("stockfish.exe"));
        candidates.push(
            current_dir
                .join("references")
                .join("Stockfish")
                .join("stockfish.exe"),
        );
    }

    if let Ok(exe_path) = env::current_exe() {
        for ancestor in exe_path.ancestors() {
            candidates.push(ancestor.join("stockfish.exe"));
            candidates.push(ancestor.join("resources").join("stockfish.exe"));
            candidates.push(ancestor.join("assets").join("bin").join("stockfish.exe"));
            candidates.push(
                ancestor
                    .join("references")
                    .join("Stockfish")
                    .join("stockfish.exe"),
            );
        }
    }

    candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            "Stockfish executable not found. Set STOCKFISH_PATH or place stockfish.exe next to the app, in the repo root, or under references/Stockfish/.".to_string()
        })
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
            )
            .add_systems(
                OnEnter(crate::core::GameState::InGame),
                warmup_xf_engine_pool,
            );

        let _pool = AsyncComputeTaskPool::get_or_init(Default::default);
    }
}

/// Pre-allocate the XFChess engine game on game entry to avoid the 2.2 GB TT
/// zero-write during the first AI move. Runs immediately after transitioning to
/// InGame while the board and assets are loading, so it finishes before the player
/// can make their first move.
fn warmup_xf_engine_pool(mut commands: Commands, ai_config: Res<ChessAIResource>) {
    if ai_config.engine != crate::game::ai::resource::AIEngine::XFChessEngine {
        return;
    }
    if matches!(
        ai_config.mode,
        crate::game::ai::resource::GameMode::Multiplayer
            | crate::game::ai::resource::GameMode::MultiplayerCompetitive
    ) {
        return;
    }

    let pool_arc = std::sync::Arc::new(std::sync::Mutex::new(None::<nimzovich_engine::Game>));
    let fill = pool_arc.clone();

    AsyncComputeTaskPool::get()
        .spawn(async move {
            let game = nimzovich_engine::game_from_fen(
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            );
            *fill.lock().unwrap() = Some(game);
        })
        .detach();

    commands.insert_resource(XFChessGamePool(pool_arc));
    info!("[AI] XFChess engine warm-up started");
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
    pub sf_process: Option<ResMut<'w, StockfishProcess>>,
    pub move_history: Res<'w, crate::game::resources::MoveHistory>,
    pub active_tc: Option<Res<'w, crate::game::resources::active_time_control::ActiveTimeControl>>,
    pub game_pool: Option<Res<'w, XFChessGamePool>>,
}

/// Compute think_time and an optional depth cap from time control context.
///
/// - Caps think_time to `base_seconds / 40` so the AI can't flag in short games.
/// - Sets `max_depth = Some(6)` for fast games with no increment (< 60 s + 0).
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

    // Remaining-time budget: base / estimated_moves_left (never less than 5)
    let think_time = if base_secs > 0 {
        let est_moves_left = (40.0 - half_moves_played as f32 / 2.0).max(5.0);
        base_think.min(base_secs as f32 / est_moves_left)
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
    pub engine: ResMut<'w, ChessEngine>,
    pub sounds: Option<Res<'w, crate::game::resources::GameSounds>>,
}

fn spawn_ai_task_system(mut commands: Commands, params: AiSpawnParams) {
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

    // FEN is already current — game_logic.rs synced the engine and built the move cache.
    let fen = params.engine.current_fen().to_string();
    let depth = params.ai_config.difficulty.stockfish_depth();
    let movetime_ms = params.ai_config.difficulty.stockfish_movetime_ms();
    let ai_color = params.ai_config.mode.ai_color();

    match params.ai_config.engine {
        crate::game::ai::resource::AIEngine::Stockfish => {
            info!("[AI] Spawning Stockfish task (persistent process)");
            let depth = depth.unwrap_or(12);
            let movetime = movetime_ms.unwrap_or(1500);

            // Get or create the persistent process Arc, then clone it for the task.
            let sf_arc = if let Some(sf) = params.sf_process.as_ref() {
                sf.0.clone()
            } else {
                match StockfishInner::new() {
                    Ok(inner) => {
                        let arc = std::sync::Arc::new(std::sync::Mutex::new(inner));
                        commands.insert_resource(StockfishProcess(arc.clone()));
                        arc
                    }
                    Err(e) => {
                        error!("[AI] Failed to start persistent Stockfish: {}", e);
                        return;
                    }
                }
            };

            let task = spawn_stockfish_task_persistent(fen, depth, movetime, sf_arc);
            commands.insert_resource(PendingAIMove(task));
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
            // Try to take the pre-warmed game from the pool to avoid re-allocating the
            // 2.2 GB transposition table on every move. Pass the pool Arc into the
            // task so it can put the game back when the search finishes.
            let pool_arc = params.game_pool.as_ref().map(|p| p.0.clone());
            let preloaded = pool_arc.as_ref().and_then(|arc| arc.lock().ok()?.take());
            let task =
                spawn_xf_engine_task(fen, think_time, max_depth, ai_color, preloaded, pool_arc);
            commands.insert_resource(PendingAIMove(task));
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

        // Return game to the pool for reuse on the next AI move.
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
        })
    })
}

/// Spawn a task that queries the persistent Stockfish process and returns the best move.
/// No cold-start overhead — the process stays alive between moves.
fn spawn_stockfish_task_persistent(
    fen: String,
    depth: u8,
    movetime_ms: u64,
    sf: std::sync::Arc<std::sync::Mutex<StockfishInner>>,
) -> Task<Result<AIMove, String>> {
    AsyncComputeTaskPool::get().spawn(async move {
        let start_time = Instant::now();

        let mut guard = sf
            .lock()
            .map_err(|e| format!("Stockfish mutex poisoned: {}", e))?;

        // One-time UCI handshake — wait for uciok, then readyok.
        if !guard.initialized {
            writeln!(guard.stdin, "uci").map_err(|e| e.to_string())?;
            guard.stdin.flush().map_err(|e| e.to_string())?;
            loop {
                let mut line = String::new();
                if guard
                    .reader
                    .read_line(&mut line)
                    .map_err(|e| e.to_string())?
                    == 0
                {
                    break;
                }
                if line.trim() == "uciok" {
                    break;
                }
            }
            writeln!(guard.stdin, "isready").map_err(|e| e.to_string())?;
            guard.stdin.flush().map_err(|e| e.to_string())?;
            loop {
                let mut line = String::new();
                if guard
                    .reader
                    .read_line(&mut line)
                    .map_err(|e| e.to_string())?
                    == 0
                {
                    break;
                }
                if line.trim() == "readyok" {
                    break;
                }
            }
            guard.initialized = true;
        }

        // Send position and search command.
        writeln!(guard.stdin, "position fen {}", fen).map_err(|e| e.to_string())?;
        guard.stdin.flush().map_err(|e| e.to_string())?;
        if movetime_ms > 0 {
            writeln!(guard.stdin, "go movetime {}", movetime_ms).map_err(|e| e.to_string())?;
        } else {
            writeln!(guard.stdin, "go depth {}", depth).map_err(|e| e.to_string())?;
        }
        guard.stdin.flush().map_err(|e| e.to_string())?;

        // Read until bestmove.
        let mut best_move = String::new();
        let mut score = 0i32;
        let mut search_depth = 0u8;
        loop {
            let mut line = String::new();
            if guard
                .reader
                .read_line(&mut line)
                .map_err(|e| e.to_string())?
                == 0
            {
                break;
            }
            let trimmed = line.trim();
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] == "info" {
                for (i, &part) in parts.iter().enumerate() {
                    if part == "depth" && i + 1 < parts.len() {
                        if let Ok(d) = parts[i + 1].parse::<u8>() {
                            search_depth = d;
                        }
                    }
                    if part == "cp" && i + 1 < parts.len() {
                        if let Ok(s) = parts[i + 1].parse::<i32>() {
                            score = s;
                        }
                    }
                }
            }
            if parts.len() >= 2 && parts[0] == "bestmove" {
                best_move = parts[1].to_string();
                break;
            }
        }

        if best_move.is_empty() || best_move == "(none)" {
            return Err("Stockfish did not return a valid move".to_string());
        }

        let thinking_time = start_time.elapsed().as_secs_f32();
        info!(
            "[AI] Stockfish: {} depth={} score={} time={:.2}s",
            best_move, search_depth, score, thinking_time
        );

        if best_move.len() >= 4 {
            let from_coords = ChessEngine::uci_to_coords(&best_move[0..2])
                .ok_or_else(|| format!("Invalid from square: {}", &best_move[0..2]))?;
            let to_coords = ChessEngine::uci_to_coords(&best_move[2..4])
                .ok_or_else(|| format!("Invalid to square: {}", &best_move[2..4]))?;
            return Ok(AIMove {
                from: from_coords,
                to: to_coords,
                uci: best_move,
                score,
                depth: search_depth,
                thinking_time,
            });
        }

        Err(format!("Invalid move format from Stockfish: {}", best_move))
    })
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
    if pending_task.is_some() {
        trace!("[AI] Skipping spawn: pending task already exists");
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

    if let crate::game::ai::resource::GameMode::Multiplayer = ai_config.mode {
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

/// System that polls the AI task and executes the move when ready
#[allow(clippy::too_many_arguments)]
fn poll_ai_task_system(mut commands: Commands, mut params: AiPollParams) {
    let Some(mut task_resource) = params.task_resource else {
        return;
    };

    let mut move_found = None;
    let mut move_from_direct_stockfish = false;

    // Poll the local AI task
    if move_found.is_none() {
        if let Some(result) =
            futures_lite::future::block_on(futures_lite::future::poll_once(&mut task_resource.0))
        {
            commands.remove_resource::<PendingAIMove>();

            match result {
                Ok(ai_move) => {
                    info!(
                        "[AI] Direct Stockfish task completed with move: {}",
                        ai_move.uci
                    );
                    let uci_move = format!(
                        "{}{}",
                        ChessEngine::coords_to_uci(ai_move.from.0, ai_move.from.1),
                        ChessEngine::coords_to_uci(ai_move.to.0, ai_move.to.1)
                    );
                    move_found = Some(uci_move);
                    move_from_direct_stockfish = true;

                    // Update AI statistics
                    params.ai_stats.last_score = ai_move.score as i64;
                    params.ai_stats.last_depth = ai_move.depth as i64;
                    params.ai_stats.thinking_time = ai_move.thinking_time;
                }
                Err(e) => {
                    error!("[AI] Stockfish task failed: {}", e);
                }
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

            if move_from_direct_stockfish {
                info!(
                    "[AI] Executing Stockfish move: {} -> {} (promotion: {:?})",
                    from_str, to_str, promotion_type
                );
            } else {
                info!(
                    "[AI] Braid sidecar yielded move: {} -> {} (promotion: {:?})",
                    from_str, to_str, promotion_type
                );
            }

            let (move_sound, capture_sound) = if let Some(s) = &params.sounds {
                (Some(s.move_piece.clone()), Some(s.capture_piece.clone()))
            } else {
                (None, None)
            };

            let mut p0 = params.pieces_queries.p0();

            // Validate with the cached legal-move table — O(1) lookup, no extra generation.
            let from_uci = ChessEngine::coords_to_uci(from_coords.0, from_coords.1);
            let to_uci = ChessEngine::coords_to_uci(to_coords.0, to_coords.1);
            let move_uci = format!("{}{}", from_uci, to_uci);

            if !params.engine.is_move_legal_by_uci(&move_uci) {
                warn!("[AI] Stockfish suggested illegal move {}", move_uci);
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
                    &mut commands,
                    &mut params.pending_turn,
                    &mut params.move_history,
                    &mut params.captured_pieces,
                    &mut params.engine,
                    &mut p0,
                    None,
                    None, // BoardStateSync not available in AI context
                    &params.current_turn,
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
