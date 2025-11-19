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

use crate::game::ai::resource::{ChessAIResource, GameMode};
use crate::game::components::GamePhase;
use crate::game::components::HasMoved;
use crate::game::resources::*;
use crate::rendering::pieces::{Piece, PieceColor};
use bevy::audio::AudioPlayer;
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
/// # Example
///
/// ```rust,ignore
/// app.add_systems(OnEnter(GameState::InGame), reset_game_resources);
/// ```
pub fn reset_game_resources(
    mut current_turn: ResMut<CurrentTurn>,
    mut game_phase: ResMut<CurrentGamePhase>,
    mut selection: ResMut<Selection>,
    mut move_history: ResMut<MoveHistory>,
    mut game_timer: ResMut<GameTimer>,
    mut captured_pieces: ResMut<CapturedPieces>,
    mut game_over: ResMut<GameOverState>,
    mut fast_board: ResMut<FastBoardState>,
    mut turn_context: ResMut<TurnStateContext>,
    mut engine: ResMut<ChessEngine>,
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

    // Reset timer to 10 minutes and start it
    *game_timer = GameTimer::default();
    game_timer.is_running = true; // Start the timer
    info!(
        "[GAME_INIT] Timer reset: {}s per player, running: {}",
        game_timer.white_time_left, game_timer.is_running
    );

    // Clear captured pieces
    captured_pieces.clear();
    info!("[GAME_INIT] Captured pieces cleared");

    // Reset game over state to Playing
    *game_over = GameOverState::Playing;
    info!("[GAME_INIT] Game over state reset: {:?}", game_over);

    // Clear fast board state (bitboards)
    fast_board.clear();
    info!("[GAME_INIT] Fast board state cleared");

    // Reset turn state context to default phase
    *turn_context = TurnStateContext::default();
    info!(
        "[GAME_INIT] Turn state context reset: {:?}",
        turn_context.phase
    );

    // Reset chess engine to starting position
    engine.reset();
    info!("[GAME_INIT] Chess engine reset to starting position");

    info!("[GAME_INIT] All game resources reset successfully - ready for new game");
}

/// System that initializes game sounds when entering InGame state
///
/// Loads all sound effect handles and stores them in the GameSounds resource.
/// This system should run after reset_game_resources to ensure sounds are
/// available for the new game.
pub fn initialize_game_sounds(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    info!("[GAME_INIT] Loading game sounds");
    let game_sounds = GameSounds::new(&asset_server);
    commands.insert_resource(game_sounds);
    info!("[GAME_INIT] Game sounds loaded successfully");
}

/// System that plays TempleOS sound when entering InGame state in TempleOS mode
///
/// Checks if ViewMode is TempleOS and plays the temple_os.mp3 sound.
/// This system should run after initialize_game_sounds to ensure the sound is loaded.
/// Verifies the audio asset is loaded before attempting to play it.
///
/// NOTE: This only runs once on state entry. For continuous checking, use `check_and_play_templeos_sound`.
pub fn play_templeos_sound(
    view_mode: Res<crate::game::view_mode::ViewMode>,
    game_sounds: Option<Res<GameSounds>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if *view_mode != crate::game::view_mode::ViewMode::TempleOS {
        return;
    }

    info!("[GAME_INIT] Attempting to play TempleOS sound on state entry");

    if let Some(sounds) = game_sounds {
        // Check if the audio asset is loaded before playing
        let load_state = asset_server.load_state(&sounds.temple_os);
        info!("[GAME_INIT] TempleOS audio load state: {:?}", load_state);
        match load_state {
            bevy::asset::LoadState::Loaded => {
                info!("[GAME_INIT] TempleOS audio loaded - spawning AudioPlayer");
                commands.spawn(AudioPlayer::new(sounds.temple_os.clone()));
                info!("[GAME_INIT] TempleOS audio playback started");
            }
            bevy::asset::LoadState::Failed(err) => {
                error!("[GAME_INIT] TempleOS audio file failed to load: {:?} - skipping playback", err);
            }
            bevy::asset::LoadState::NotLoaded => {
                warn!("[GAME_INIT] TempleOS audio not yet loaded - will be handled by continuous check system");
            }
            bevy::asset::LoadState::Loading => {
                info!("[GAME_INIT] TempleOS audio still loading - will be handled by continuous check system");
            }
        }
    } else {
        warn!("[GAME_INIT] GameSounds resource not available - cannot play TempleOS sound");
    }
}

/// Resource to track if TempleOS sound has been played
///
/// Prevents multiple plays of the same sound when the asset loads.
#[derive(Resource, Default)]
pub struct TempleOSSoundPlayed {
    pub played: bool,
}

/// Continuous system that checks and plays TempleOS sound when asset is loaded
///
/// This system runs every frame in Update schedule and checks if the TempleOS
/// audio asset has finished loading. Once loaded, it plays the sound once.
///
/// This handles the case where the asset is still loading when entering InGame state.
pub fn check_and_play_templeos_sound(
    view_mode: Res<crate::game::view_mode::ViewMode>,
    game_sounds: Option<Res<GameSounds>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    sound_played: Option<ResMut<TempleOSSoundPlayed>>,
) {
    // Only check in TempleOS mode
    if *view_mode != crate::game::view_mode::ViewMode::TempleOS {
        return;
    }

    // Initialize resource if it doesn't exist
    if sound_played.is_none() {
        commands.init_resource::<TempleOSSoundPlayed>();
        return;
    }

    // Check if we've already played the sound
    let already_played = sound_played.as_ref().map(|p| p.played).unwrap_or(false);
    if already_played {
        return; // Already played, don't play again
    }

    // Check if sound is loaded and play it
    if let Some(sounds) = game_sounds {
        let load_state = asset_server.load_state(&sounds.temple_os);
        match load_state {
            bevy::asset::LoadState::Loaded => {
                info!("[AUDIO] TempleOS audio loaded - playing now");
                commands.spawn(AudioPlayer::new(sounds.temple_os.clone()));
                // Mark as played
                if let Some(mut played) = sound_played {
                    played.played = true;
                }
                info!("[AUDIO] TempleOS audio playback started");
            }
            bevy::asset::LoadState::Failed(err) => {
                error!("[AUDIO] TempleOS audio failed to load: {:?}", err);
                // Mark as played to prevent repeated error logs
                if let Some(mut played) = sound_played {
                    played.played = true;
                }
            }
            bevy::asset::LoadState::NotLoaded => {
                debug!("[AUDIO] TempleOS audio not yet loaded");
            }
            bevy::asset::LoadState::Loading => {
                debug!("[AUDIO] TempleOS audio still loading...");
            }
        }
    } else {
        debug!("[AUDIO] GameSounds resource not yet available");
    }
}

/// System that initializes players based on game mode
///
/// Creates player resources based on the ChessAIResource mode:
/// - VsHuman: Both players are human
/// - VsAI: One human, one AI (based on ai_color)
///
/// This system runs when entering InGame state to set up players.
pub fn initialize_players(mut players: ResMut<Players>, ai_config: Res<ChessAIResource>) {
    info!(
        "[GAME_INIT] Initializing players based on game mode: {:?}",
        ai_config.mode
    );

    match ai_config.mode {
        GameMode::VsHuman => {
            // Both players are human
            *players = Players {
                player_1: Player::new(1, "Player 1".to_string(), PieceColor::White, true),
                player_2: Player::new(2, "Player 2".to_string(), PieceColor::Black, true),
            };
            info!("[GAME_INIT] Players initialized: Human vs Human");
        }
        GameMode::VsAI { ai_color } => {
            // One human, one AI
            let human_color = match ai_color {
                PieceColor::White => PieceColor::Black,
                PieceColor::Black => PieceColor::White,
            };

            *players = Players {
                player_1: Player::new(
                    1,
                    if human_color == PieceColor::White {
                        "Player 1".to_string()
                    } else {
                        "AI".to_string()
                    },
                    PieceColor::White,
                    human_color == PieceColor::White,
                ),
                player_2: Player::new(
                    2,
                    if ai_color == PieceColor::Black {
                        "AI".to_string()
                    } else {
                        "Player 1".to_string()
                    },
                    PieceColor::Black,
                    ai_color != PieceColor::Black,
                ),
            };
            info!(
                "[GAME_INIT] Players initialized: Human ({:?}) vs AI ({:?})",
                human_color, ai_color
            );
        }
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
/// # Example
///
/// ```rust,ignore
/// app.add_systems(OnEnter(GameState::InGame), (
///     reset_game_resources,
///     create_pieces,
///     initialize_engine_from_ecs.after(create_pieces),
/// ).chain());
/// ```
pub fn initialize_engine_from_ecs(
    mut engine: ResMut<ChessEngine>,
    pieces_query: Query<(Entity, &Piece, &HasMoved)>,
    current_turn: Res<CurrentTurn>,
    view_mode: Res<crate::game::view_mode::ViewMode>,
) {
    info!("[GAME_INIT] Initializing chess engine from ECS board state");
    info!("[GAME_INIT] View mode: {:?}", *view_mode);

    // Skip engine initialization in TempleOS mode (no pieces, no game)
    if *view_mode == crate::game::view_mode::ViewMode::TempleOS {
        info!("[GAME_INIT] Skipping engine initialization - TempleOS mode (no game logic)");
        return;
    }

    // Sync ECS board state to engine
    engine.sync_ecs_to_engine(&pieces_query, &current_turn);

    let piece_count = pieces_query.iter().count();
    info!("[GAME_INIT] Engine initialized with {} pieces", piece_count);
    info!(
        "[GAME_INIT] Engine move counter: {}",
        engine.game.move_counter
    );
    info!("[GAME_INIT] Engine ready for move validation");
}
