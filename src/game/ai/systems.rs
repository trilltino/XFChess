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
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

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
}

fn spawn_ai_task_system(
    mut commands: Commands,
    mut params: AiSpawnParams,
    braid_manager: Option<Res<crate::multiplayer::network::braid::BraidNodeManager>>,
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

    // Check if BraidNodeManager has Stockfish sidecar properly initialized
    let has_stockfish_sidecar = braid_manager
        .as_ref()
        .and_then(|bm| bm.sidecar_fen_tx.as_ref())
        .is_some();

    if has_stockfish_sidecar {
        let braid_manager = braid_manager.unwrap();
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
        // No BraidNodeManager or sidecar not initialized - spawn Stockfish directly
        info!("[AI] BraidNodeManager unavailable or sidecar not initialized, spawning Stockfish process directly");
        
        let depth = depth.unwrap_or(12);
        let movetime = movetime_ms.unwrap_or(1500);
        
        let task = spawn_stockfish_task(fen, depth, movetime);
        commands.insert_resource(PendingAIMove(task));
    }
}

/// Spawn a task that runs Stockfish process and returns the best move
fn spawn_stockfish_task(fen: String, depth: u8, movetime_ms: u64) -> Task<Result<AIMove, String>> {
    AsyncComputeTaskPool::get().spawn(async move {
        let start_time = Instant::now();
        
        // Try to find stockfish executable in common locations
        let stockfish_paths = [
            "assets/bin/stockfish.exe",
            "assets/bin/stockfish",
            "stockfish.exe",
            "stockfish",
            "references/Stockfish/stockfish.exe",
            "references/Stockfish/stockfish",
        ];
        
        let stockfish_path = stockfish_paths.iter()
            .find(|p| std::path::Path::new(p).exists())
            .ok_or_else(|| "Stockfish executable not found. Please install Stockfish or place stockfish.exe in the project root.".to_string())?;
        
        info!("[AI] Starting Stockfish process at: {}", stockfish_path);
        
        let mut child = Command::new(stockfish_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn Stockfish: {}", e))?;
        
        let mut stdin = child.stdin.take().ok_or("Failed to get stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to get stdout")?;
        let mut reader = BufReader::new(stdout);
        
        // Send UCI commands to Stockfish
        writeln!(stdin, "uci").map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(100));
        
        writeln!(stdin, "isready").map_err(|e| e.to_string())?;
        
        // Wait for readyok
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).map_err(|e| e.to_string())?;
            if bytes_read == 0 {
                break;
            }
            if line.trim() == "readyok" {
                break;
            }
        }
        
        // Set position and search
        writeln!(stdin, "position fen {}", fen).map_err(|e| e.to_string())?;
        
        // Use go command with movetime or depth
        if movetime_ms > 0 {
            writeln!(stdin, "go movetime {}", movetime_ms).map_err(|e| e.to_string())?;
        } else {
            writeln!(stdin, "go depth {}", depth).map_err(|e| e.to_string())?;
        }
        
        // Wait for best move
        let mut best_move = String::new();
        let mut score = 0i32;
        let mut search_depth = 0u8;
        
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).map_err(|e| e.to_string())?;
            if bytes_read == 0 {
                break;
            }
            
            let line = line.trim();
            let parts: Vec<&str> = line.split_whitespace().collect();
            
            if parts.len() >= 2 && parts[0] == "info" {
                // Parse score and depth from info lines
                for (i, part) in parts.iter().enumerate() {
                    if *part == "depth" && i + 1 < parts.len() {
                        if let Ok(d) = parts[i + 1].parse::<u8>() {
                            search_depth = d;
                        }
                    }
                    if *part == "cp" && i + 1 < parts.len() {
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
        
        // Quit Stockfish
        let _ = writeln!(stdin, "quit");
        let _ = child.wait();
        
        if best_move.is_empty() || best_move == "(none)" {
            return Err("Stockfish did not return a valid move".to_string());
        }
        
        let thinking_time = start_time.elapsed().as_secs_f32();
        info!("[AI] Stockfish returned: {} (depth: {}, score: {}, time: {:.2}s)", 
              best_move, search_depth, score, thinking_time);
        
        // Parse UCI move (e.g., "e2e4")
        if best_move.len() >= 4 {
            let from_str = &best_move[0..2];
            let to_str = &best_move[2..4];
            let promotion_char = best_move.chars().nth(4);
            
            let from_coords = ChessEngine::uci_to_coords(from_str)
                .ok_or_else(|| format!("Invalid from square: {}", from_str))?;
            let to_coords = ChessEngine::uci_to_coords(to_str)
                .ok_or_else(|| format!("Invalid to square: {}", to_str))?;
            
            return Ok(AIMove {
                from: from_coords,
                to: to_coords,
                uci: best_move.clone(),
                score,
                depth: search_depth,
                thinking_time,
                fen_after: String::new(), // Will be computed after move
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
        debug!("[AI] Skipping spawn: pending task already exists");
        return true;
    }
    
    if pending_turn_advance
            .as_ref()
            .map(|r| r.is_pending())
            .unwrap_or(false) {
        debug!("[AI] Skipping spawn: turn advance pending");
        return true;
    }

    if let crate::game::ai::resource::GameMode::Multiplayer = ai_config.mode {
        debug!("[AI] Skipping spawn: game mode is Multiplayer");
        return true;
    }

    if game_phase.0 != GamePhase::Playing {
        debug!("[AI] Skipping spawn: game phase is {:?}, not Playing", game_phase.0);
        return true;
    }

    let current_player = players.current(current_turn.color);
    if current_player.is_human {
        info!("[AI] Skipping spawn: current player ({:?}) is human", current_turn.color);
        return true;
    }

    let ai_color = ai_config.mode.ai_color();
    if current_turn.color != ai_color {
        info!("[AI] Skipping spawn: turn color ({:?}) != AI color ({:?})", current_turn.color, ai_color);
        return true;
    }

    info!("[AI] Conditions met - spawning AI task for {:?}", current_turn.color);
    false
}

/// System that polls the AI task and executes the move when ready
#[allow(clippy::too_many_arguments)]
fn poll_ai_task_system(
    mut commands: Commands,
    mut params: AiPollParams,
    braid_manager: Option<Res<crate::multiplayer::network::braid::BraidNodeManager>>,
) {
    let Some(mut task_resource) = params.task_resource else {
        return;
    };

    let mut move_found = None;
    let mut move_from_direct_stockfish = false;

    // Check Braid channel first (if available)
    if let Some(braid_manager) = &braid_manager {
        if let Some(rx) = &braid_manager.incoming_moves_rx {
            if let Ok(alg_move) = rx.try_recv() {
                move_found = Some(alg_move);
                commands.remove_resource::<PendingAIMove>();
            }
        }
    }

    // If no move from Braid, check the async task (for direct Stockfish execution)
    if move_found.is_none() {
        if let Some(result) = futures_lite::future::block_on(futures_lite::future::poll_once(&mut task_resource.0)) {
            commands.remove_resource::<PendingAIMove>();
            
            match result {
                Ok(ai_move) => {
                    info!("[AI] Direct Stockfish task completed with move: {}", ai_move.uci);
                    let uci_move = format!("{}{}", 
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
