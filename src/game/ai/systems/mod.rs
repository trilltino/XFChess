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
use std::time::{Duration, Instant};
use xfchess_stockfish::{EnginePosition, SearchRequest, SearchTicket};

mod stockfish;
pub use stockfish::*;

#[derive(Resource)]
pub struct PendingAIMove {
    pub ticket: SearchTicket,
    fen: String,
    game_id: u64,
    spawned_at: Instant,
    min_reveal_delay: Duration,
}

#[derive(Resource)]
pub struct AIMovePendingReveal {
    result: Result<AIMove, String>,
    ready_at: Instant,
    fen: String,
    game_id: u64,
}

#[derive(Resource, Default)]
pub struct AIEngineStatus {
    pub game_id: u64,
    pub error: Option<String>,
    pub clock_was_running: bool,
}

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
        use crate::core::state::{GameMode, InGameplay};

        // The AI only plays in vs-AI games; replay, spectating, puzzles and
        // online games never start a search.
        app.init_resource::<ChessAIResource>()
            .init_resource::<AIStatistics>()
            .init_resource::<AIEngineStatus>()
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
                (begin_ai_game, warm_up_stockfish)
                    .chain()
                    .after(clear_pending_ai_tasks)
                    .run_if(resource_equals(GameMode::SinglePlayer)),
            )
            // A search still running when the player leaves is dropped, not applied
            // to whatever is on the board next.
            .add_systems(
                OnExit(InGameplay),
                (clear_pending_ai_tasks, stop_stockfish).chain(),
            )
            .add_systems(
                OnEnter(crate::core::GameState::GameOver),
                clear_pending_ai_tasks,
            );

        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            engine_error_dialog.run_if(resource_equals(GameMode::SinglePlayer)),
        );
    }
}

fn begin_ai_game(mut status: ResMut<AIEngineStatus>, mut commands: Commands) {
    status.game_id = status.game_id.wrapping_add(1);
    status.error = None;
    status.clock_was_running = false;
    commands.remove_resource::<PendingAIMove>();
    commands.remove_resource::<AIMovePendingReveal>();
}

pub fn clear_pending_ai_tasks(mut commands: Commands, status: Option<ResMut<AIEngineStatus>>) {
    commands.remove_resource::<PendingAIMove>();
    commands.remove_resource::<AIMovePendingReveal>();
    if let Some(mut status) = status {
        status.game_id = status.game_id.wrapping_add(1);
        status.error = None;
    }
}

fn stop_stockfish(mut commands: Commands) {
    commands.remove_resource::<StockfishProcess>();
}

fn engine_error_dialog(
    mut contexts: bevy_egui::EguiContexts,
    mut status: ResMut<AIEngineStatus>,
    mut timer: Option<ResMut<crate::game::resources::GameTimer>>,
    mut next_state: ResMut<NextState<crate::core::GameState>>,
    mut commands: Commands,
) {
    let Some(error) = status.error.clone() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    bevy_egui::egui::Window::new("Stockfish unavailable")
        .collapsible(false)
        .resizable(false)
        .anchor(
            bevy_egui::egui::Align2::CENTER_CENTER,
            bevy_egui::egui::Vec2::ZERO,
        )
        .show(ctx, |ui| {
            ui.label("The game is paused because Stockfish could not complete its move.");
            ui.label(error);
            if ui.button("Retry").clicked() {
                status.error = None;
                if let Some(timer) = timer.as_mut() {
                    timer.is_running = status.clock_was_running;
                }
                commands.remove_resource::<StockfishProcess>();
            }
            if ui.button("Return to menu").clicked() {
                next_state.set(crate::core::GameState::MainMenu);
            }
        });
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
    pub status: Res<'w, AIEngineStatus>,
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
    pub status: ResMut<'w, AIEngineStatus>,
    pub timer: Option<ResMut<'w, crate::game::resources::GameTimer>>,
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

    if params.status.error.is_some() {
        return;
    }
    let fen = params.engine.current_fen().to_string();
    let (think_time, max_depth) = compute_think_params(
        params.ai_config.difficulty.seconds_per_move(),
        params.move_history.len(),
        params.active_tc.as_deref(),
    );
    let depth = params
        .ai_config
        .difficulty
        .stockfish_depth()
        .map(|d| max_depth.map_or(d, |cap| cap.min(d)));
    let process = match params
        .sf_process
        .as_deref()
        .cloned()
        .map(Ok)
        .unwrap_or_else(StockfishProcess::start)
    {
        Ok(process) => process,
        Err(error) => {
            commands.queue(move |world: &mut World| {
                world.resource_mut::<AIEngineStatus>().error = Some(error);
            });
            return;
        }
    };
    commands.insert_resource(process.clone());
    let mut position = EnginePosition::from_fen(xfchess_rules::STARTING_FEN);
    position.moves = params
        .move_history
        .iter()
        .map(|record| record.uci())
        .collect();
    // Custom starts and restored boards may not have a complete local move log.
    if position.game().map(|g| g.fen()).as_deref() != Ok(fen.as_str()) {
        position = EnginePosition::from_fen(fen.clone());
    }
    let ticket = process.0.search(SearchRequest {
        game_id: params.status.game_id,
        position,
        movetime_ms: (think_time * 1000.0).round().max(1.0) as u64,
        depth,
        strength_elo: params.ai_config.difficulty.stockfish_elo(),
    });
    let (min, max) = params.ai_config.difficulty.thinking_delay_range_ms();
    commands.insert_resource(PendingAIMove {
        ticket,
        fen,
        game_id: params.status.game_id,
        spawned_at: Instant::now(),
        min_reveal_delay: Duration::from_millis(rand::random_range(min..=max)),
    });
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
    if params.status.error.is_some() {
        if let Some(timer) = params.timer.as_mut() {
            status_pause_clock(&mut params.status, timer);
        }
        return;
    }
    if let Some(reveal) = params.pending_reveal.as_ref() {
        if reveal.game_id != params.status.game_id || reveal.fen != params.engine.current_fen() {
            commands.remove_resource::<AIMovePendingReveal>();
            return;
        }
        if Instant::now() < reveal.ready_at {
            return;
        }
        let result = reveal.result.clone();
        commands.remove_resource::<AIMovePendingReveal>();
        apply_ai_result(result, &mut commands, &mut params);
        return;
    }
    let Some(task) = params.task_resource.as_ref() else {
        return;
    };
    if task.game_id != params.status.game_id || task.fen != params.engine.current_fen() {
        commands.remove_resource::<PendingAIMove>();
        return;
    }
    let Some(result) = task.ticket.try_result() else {
        return;
    };
    let result = result.and_then(|result| {
        let uci = result.best_move.ok_or("Stockfish returned no move")?;
        let (from, to, _) = ChessEngine::parse_uci_move(&uci).ok_or("Invalid Stockfish move")?;
        Ok(AIMove {
            from,
            to,
            uci,
            score: result.score.map_or(0, |s| s.centipawns()),
            depth: result.depth,
            nodes: result.nodes,
            thinking_time: result.elapsed,
        })
    });
    let ready_at = task.spawned_at + task.min_reveal_delay;
    let fen = task.fen.clone();
    let game_id = task.game_id;
    commands.remove_resource::<PendingAIMove>();
    if Instant::now() >= ready_at || result.is_err() {
        apply_ai_result(result, &mut commands, &mut params);
    } else {
        commands.insert_resource(AIMovePendingReveal {
            result,
            ready_at,
            fen,
            game_id,
        });
    }
}

fn status_pause_clock(status: &mut AIEngineStatus, timer: &mut crate::game::resources::GameTimer) {
    if timer.is_running {
        status.clock_was_running = true;
        timer.is_running = false;
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
            info!("[AI] Stockfish search finished: {}", ai_move.uci);
            move_found = Some(ai_move.uci);

            params.ai_stats.last_score = ai_move.score as i64;
            params.ai_stats.last_depth = ai_move.depth as i64;
            params.ai_stats.last_nodes = ai_move.nodes;
            params.ai_stats.thinking_time = ai_move.thinking_time;
        }
        Err(e) => {
            error!("[AI] Stockfish task failed: {}", e);
            params.status.error = Some(e);
            if let Some(timer) = params.timer.as_mut() {
                status_pause_clock(&mut params.status, timer);
            }
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
                params.status.error = Some(format!("Stockfish move no longer legal: {move_uci}"));
                if let Some(timer) = params.timer.as_mut() {
                    status_pause_clock(&mut params.status, timer);
                }
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
                params.status.error =
                    Some(format!("No board piece for Stockfish move {best_move_str}"));
                if let Some(timer) = params.timer.as_mut() {
                    status_pause_clock(&mut params.status, timer);
                }
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
    use crate::core::state::{GameMode, GameState};

    use crate::game::resources::player::{Player, Players};
    use crate::rendering::pieces::PieceColor;

    /// A game entered in `mode`, with Stockfish as Black to move.
    fn game_with_ai_to_move(mode: GameMode) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<GameState>()
            .insert_resource(mode)
            .insert_resource(ChessAIResource {
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
        }
    }

    #[test]
    fn promotion_suffixes_are_preserved_in_uci() -> Result<(), Box<dyn std::error::Error>> {
        let from = ChessEngine::uci_to_coords("e7").ok_or("missing test value")?;
        let to = ChessEngine::uci_to_coords("e8").ok_or("missing test value")?;
        for promotion in ['q', 'r', 'b', 'n'] {
            assert_eq!(
                normalized_move_uci(from, to, Some(promotion)),
                format!("e7e8{promotion}")
            );
        }

        Ok(())
    }

    fn poll_app() -> App {
        let mut app = App::new();
        app.init_resource::<ChessAIResource>()
            .init_resource::<AIEngineStatus>()
            .init_resource::<AIStatistics>()
            .init_resource::<CurrentTurn>()
            .init_resource::<MoveHistory>()
            .init_resource::<CapturedPieces>()
            .init_resource::<crate::game::resources::PendingTurnAdvance>()
            .init_resource::<crate::game::resources::GameTimer>()
            .init_resource::<ChessEngine>()
            .add_message::<MoveMadeEvent>()
            .add_systems(Update, poll_ai_task_system);
        app
    }

    #[test]
    fn results_from_a_previous_game_or_position_are_discarded() {
        for (game_id, fen) in [
            (99, xfchess_rules::STARTING_FEN),
            (0, "a different position"),
        ] {
            let mut app = poll_app();
            app.insert_resource(AIMovePendingReveal {
                result: Err("old worker error".into()),
                ready_at: Instant::now(),
                fen: fen.into(),
                game_id,
            });
            app.update();
            assert!(!app.world().contains_resource::<AIMovePendingReveal>());
            assert!(app.world().resource::<AIEngineStatus>().error.is_none());
            assert!(app.world().resource::<MoveHistory>().is_empty());
        }
    }

    #[test]
    fn failed_search_pauses_clock_and_does_not_create_another_search() {
        let mut app = poll_app();
        app.world_mut()
            .resource_mut::<crate::game::resources::GameTimer>()
            .is_running = true;
        app.insert_resource(AIMovePendingReveal {
            result: Err("Stockfish failed twice".into()),
            ready_at: Instant::now(),
            fen: xfchess_rules::STARTING_FEN.into(),
            game_id: 0,
        });
        app.update();
        assert!(
            !app.world()
                .resource::<crate::game::resources::GameTimer>()
                .is_running
        );
        assert!(app.world().resource::<AIEngineStatus>().clock_was_running);
        assert!(app.world().resource::<AIEngineStatus>().error.is_some());
        app.update();
        assert!(!app.world().contains_resource::<PendingAIMove>());
    }
}
