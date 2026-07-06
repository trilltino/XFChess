//! Game initialization system for resetting resources on new game
//!
//! This system runs when entering the `InGame` state to ensure all game resources
//! are reset to their default values, providing a clean slate for each new game.
//!
//! # Bevy Pattern
//!
//! Follows the pattern from `reference/bevy/examples/state/states.rs` where
//! `setup_game` runs `OnEnter(AppState::InGame)` to initialize game state.
//!
//! # System Execution
//!
//! This system runs `OnEnter(GameState::InGame)` and must execute BEFORE
//! piece and board spawning systems to ensure a clean state.
//!
//! # Resources Reset
//!
//! - `CurrentTurn` - Reset to White, move 1
//! - `CurrentGamePhase` - Reset to Playing
//! - `Selection` - Clear selected piece
//! - `MoveHistory` - Clear all moves
//! - `GameTimer` - Reset to 10 minutes, start timer
//! - `CapturedPieces` - Clear all captures
//! - `GameOverState` - Reset to Playing
//! - `FastBoardState` - Clear bitboards
//! - `TurnStateContext` - Reset to default phase
use crate::core::{DespawnOnExit, GameState};

use crate::engine::board_state::ChessEngine;
use crate::game::ai::resource::ChessAIResource;
use crate::game::components::GamePhase;
use crate::game::components::HasMoved;
use crate::game::resources::*;
use crate::rendering::pieces::{Piece, PieceColor};
use bevy::prelude::*;

/// System that resets all game resources when entering InGame state
///
/// This ensures each new game starts with clean state, preventing
/// resource persistence from previous games.
///
/// # Execution Order
///
/// This system must run BEFORE piece/board spawning to ensure
/// resources are reset before entities are created.
///
/// For usage examples, see `tests/systems_tests.rs`
pub fn reset_game_resources(
    mut commands: Commands,
    mut current_turn: ResMut<CurrentTurn>,
    mut game_phase: ResMut<CurrentGamePhase>,
    mut selection: ResMut<Selection>,
    mut move_history: ResMut<MoveHistory>,
    mut game_timer: ResMut<GameTimer>,
    mut captured_pieces: ResMut<CapturedPieces>,
    mut game_over: ResMut<GameOverState>,
    mut turn_context: ResMut<TurnStateContext>,
    mut engine: ResMut<ChessEngine>,
    active_tc: Res<crate::game::resources::active_time_control::ActiveTimeControl>,
) {
    info!("[GAME_INIT] Resetting all game resources for new game");

    // Reset turn state to White's turn, move 1
    *current_turn = CurrentTurn::default();
    info!(
        "[GAME_INIT] Turn reset: {:?}, move {}",
        current_turn.color, current_turn.move_number
    );

    // Reset game phase to Playing
    *game_phase = CurrentGamePhase(GamePhase::Playing);
    info!("[GAME_INIT] Game phase reset: {:?}", game_phase.0);

    // Clear selection (no piece selected)
    selection.clear();
    info!("[GAME_INIT] Selection cleared");

    // Clear move history
    move_history.clear();
    info!(
        "[GAME_INIT] Move history cleared (was {} moves)",
        move_history.len()
    );

    // Reset timer from the chosen time control; start is deferred until pieces are present.
    let base = active_tc.control.base_seconds() as f32;
    let inc = active_tc.control.increment_seconds() as f32;
    *game_timer = if base > 0.0 {
        GameTimer {
            white_time_left: base,
            black_time_left: base,
            increment: inc,
            is_running: false,
        }
    } else {
        GameTimer {
            white_time_left: f32::MAX,
            black_time_left: f32::MAX,
            increment: 0.0,
            is_running: false,
        }
    };
    info!(
        "[GAME_INIT] Timer reset: {}s per player (+{}s inc), waiting for pieces",
        game_timer.white_time_left, game_timer.increment
    );

    // Clear captured pieces
    captured_pieces.clear();
    info!("[GAME_INIT] Captured pieces cleared");

    // Reset game over state to Playing
    *game_over = GameOverState::Playing;
    info!("[GAME_INIT] Game over state reset: {:?}", game_over);

    // Reset turn state context to default phase
    *turn_context = TurnStateContext::default();

    // Spawn overhead light (invisible source) - "Angel Light"
    // Use high intensity to illuminate board clearly from top-down view
    commands
        .spawn(PointLight {
            intensity: 2_000_000.0, // High intensity for clear visibility
            range: 100.0,
            shadow_maps_enabled: true,
            ..Default::default()
        })
        .insert(Transform::from_xyz(3.5, 20.0, 3.5))
        .insert(GlobalTransform::default())
        .insert(DespawnOnExit(GameState::InGame))
        .insert(Name::new("Overhead Light")); // Helpful for debugging
    info!(
        "[GAME_INIT] Turn state context reset: {:?}",
        turn_context.phase
    );

    // Reset chess engine to starting position
    engine.reset();
    info!("[GAME_INIT] Chess engine reset to starting position");

    info!("[GAME_INIT] All game resources reset successfully - ready for new game");
}

/// System that initializes players based on game mode
///
/// Creates player resources based on:
/// - OnlineMultiplayer: local player is human, remote player is not (color from P2PConnectionState)
/// - MultiplayerLocal: both players human
/// - VsAI: one human, one AI (based on ai_color)
///
/// This system runs when entering InGame state to set up players.
pub fn initialize_players(
    _commands: Commands,
    mut players: ResMut<Players>,
    ai_config: Res<ChessAIResource>,
    core_mode: Res<crate::core::states::GameMode>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
) {
    if let crate::core::states::GameMode::MultiplayerLocal = *core_mode {
        *players = Players {
            player_1: Player::new(1, "Player 1".to_string(), PieceColor::White, true),
            player_2: Player::new(2, "Player 2".to_string(), PieceColor::Black, true),
        };
        info!("[GAME_INIT] Local PvP players initialized (both human)");
    } else if let crate::core::states::GameMode::OnlineMultiplayer = *core_mode {
        // Each instance only controls its assigned color; the other color is driven by the network.
        let my_color = p2p_conn
            .as_ref()
            .and_then(|s| s.player_color)
            .unwrap_or(PieceColor::White);
        *players = match my_color {
            PieceColor::White => Players {
                player_1: Player::new(1, "You (Host)".to_string(), PieceColor::White, true),
                player_2: Player::new(2, "Opponent".to_string(), PieceColor::Black, false),
            },
            PieceColor::Black => Players {
                player_1: Player::new(1, "Opponent".to_string(), PieceColor::White, false),
                player_2: Player::new(2, "You".to_string(), PieceColor::Black, true),
            },
        };
        info!(
            "[GAME_INIT] OnlineMultiplayer players initialized: local={:?}",
            my_color
        );
    } else {
        // VsAI mode: One human, one AI
        let ai_color = ai_config.mode.ai_color();
        let human_color = match ai_color {
            PieceColor::White => PieceColor::Black,
            PieceColor::Black => PieceColor::White,
        };

        *players = match ai_color {
            PieceColor::White => Players {
                player_1: Player::new(1, "AI".to_string(), PieceColor::White, false),
                player_2: Player::new(2, "Player 1".to_string(), PieceColor::Black, true),
            },
            PieceColor::Black => Players {
                player_1: Player::new(1, "Player 1".to_string(), PieceColor::White, true),
                player_2: Player::new(2, "AI".to_string(), PieceColor::Black, false),
            },
        };

        info!(
            "[GAME_INIT] VsAI players initialized: Human ({:?}) vs AI ({:?})",
            human_color, ai_color
        );
    }

    info!(
        "[GAME_INIT] Player 1: {} ({:?}, human: {})",
        players.player_1.name, players.player_1.color, players.player_1.is_human
    );
    info!(
        "[GAME_INIT] Player 2: {} ({:?}, human: {})",
        players.player_2.name, players.player_2.color, players.player_2.is_human
    );
}

/// Deferred timer start: enable the game timer only once pieces are present in ECS.
///
/// This prevents clock time being consumed during asset loading between
/// `OnEnter(InGame)` and when the board is actually interactive.
/// Uses a small frame-counter guard so the timer doesn't start on the same
/// frame pieces are spawned (avoids 1-frame edge cases).
pub fn start_timer_when_ready(
    mut game_timer: ResMut<GameTimer>,
    mut engine: ResMut<ChessEngine>,
    pieces_query: Query<(Entity, &Piece, &HasMoved)>,
    game_phase: Res<crate::game::resources::CurrentGamePhase>,
    move_history: Res<crate::game::resources::MoveHistory>,
    mut engine_inited: Local<bool>,
) {
    if game_timer.is_running {
        *engine_inited = false; // reset for next game
        return;
    }

    if game_phase.0 != crate::game::components::GamePhase::Playing {
        return;
    }

    // Sync the engine from the ECS board exactly once, as soon as the full set
    // of pieces is present. This must happen regardless of whether the clock has
    // started yet, so move validation is ready before the first move.
    if pieces_query.iter().count() >= 32 && !*engine_inited {
        engine.sync_ecs_to_engine(&pieces_query);
        *engine_inited = true;
        info!(
            "[GAME_INIT] Engine initialised with {} pieces | FEN: {}",
            pieces_query.iter().count(),
            engine.current_fen()
        );
    }

    // The clock does not start until the first move has actually been played.
    // This keeps either player's clock from ticking during the lobby/handshake
    // (e.g. while the host waits to begin) and gives the side to move its first
    // move "for free", matching the agreed multiplayer behaviour.
    if *engine_inited && !move_history.is_empty() {
        game_timer.is_running = true;
        info!("[GAME_INIT] Timer started after first move played");
    }
}

/// System that initializes the chess engine from ECS board state
///
/// This system runs AFTER pieces are spawned to sync the engine's internal
/// board state with the ECS piece positions. This establishes the engine
/// as the authoritative source for move validation.
///
/// # Execution Order
///
/// This system must run AFTER `create_pieces` to ensure pieces exist in ECS
/// before syncing to the engine.
///
/// For usage examples, see `tests/systems_tests.rs`
pub fn initialize_engine_from_ecs(
    mut engine: ResMut<ChessEngine>,
    pieces_query: Query<(Entity, &Piece, &HasMoved)>,
    view_mode: Res<crate::game::view_mode::ViewMode>,
) {
    info!("[GAME_INIT] Initializing chess engine from ECS board state");
    info!("[GAME_INIT] View mode: {:?}", *view_mode);

    // Skip engine initialization in TempleOS mode (no pieces, no game)
    if view_mode.is_templeos() {
        info!("[GAME_INIT] Skipping engine initialization - TempleOS mode (no game logic)");
        return;
    }

    // Sync ECS board state to engine
    engine.sync_ecs_to_engine(&pieces_query);

    let piece_count = pieces_query.iter().count();
    info!("[GAME_INIT] Engine initialized with {} pieces", piece_count);
    info!("[GAME_INIT] Engine FEN: {}", engine.current_fen());
    info!("[GAME_INIT] Engine ready for move validation");
}
