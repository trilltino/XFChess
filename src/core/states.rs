//! Enhanced game state system for XFChess
//!
//! Implements a comprehensive state machine following Bevy 0.17 best practices,
//! providing smooth game flow from splash screen through gameplay to game over.
//!
//! # State Flow
//!
//! ```text
//! [MainMenu] ⇄ [Settings]
//!      ↓
//!  [InGame] ⇄ [Paused]
//!      ↓
//!  [GameOver] → [MainMenu]
//! ```
//!
//! # State Descriptions
//!
//! - **MainMenu**: Main menu with game mode selection and settings (starting state)
//! - **Settings**: Configuration screen (AI difficulty, graphics, audio)
//! - **InGame**: Active chess gameplay
//! - **Paused**: In-game pause menu
//! - **GameOver**: Post-game statistics and rematch options
//!
//! # Bevy 0.17 Features
//!
//! - `States` derive macro for automatic state management
//! - `ComputedStates` for conditional system execution
//! - `SubStates` for hierarchical state relationships
//! - State transitions via `NextState<GameState>`
//!
//! # Reference
//!
//! Pattern based on:
//! - `reference/bevy/examples/games/game_menu.rs` - Multi-state game flow
//! - `reference/bevy/examples/state/states.rs` - Modern state system
//! - `reference/bevy/examples/state/sub_states.rs` - Hierarchical states

use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

/// Primary game state controlling major application modes
///
/// This is the root state that controls the overall flow of the application.
/// Each state has its own plugin that manages setup, update, and cleanup.
#[derive(Clone, Copy, Resource, PartialEq, Eq, Hash, Debug, Default, States, Reflect)]
pub enum GameState {
    /// Authentication state (Login/Register)
    #[default]
    Auth,

    /// Main menu state (starting state)
    ///
    /// Displays game mode selection, settings access, and exit options.
    /// Background shows animated 3D scene (rotating board).
    MainMenu,

    /// Settings/configuration menu
    ///
    /// Allows players to adjust AI difficulty, graphics settings, and controls.
    /// Can be accessed from MainMenu or Paused states.
    Settings,

    /// Active gameplay state
    ///
    /// Chess game in progress with full UI (timer, captures, moves).
    /// Can transition to Paused or GameOver states.
    InGame,

    /// Paused game state
    ///
    /// Game is paused, showing pause menu overlay.
    /// Can resume to InGame or quit to MainMenu.
    Paused,

    /// Game over state
    ///
    /// Shows winner, statistics, and post-game options.
    /// Displays move history and material balance.
    /// Can start new game or return to MainMenu.
    GameOver,

    /// Multiplayer Lobby Menu
    ///
    /// Host/Join UI for online play.
    MultiplayerMenu,

    /// Matching state (Connecting/Handshake)
    Matching,
}

/// Component marking entities to be despawned when exiting a specific state
///
/// Use this to automatically clean up entities associated with a specific game state.
/// The state lifecycle systems will query for this component and despawn entities
/// when their associated state is exited.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DespawnOnExit<T>(pub T)
where
    T: States + Copy;

/// Sub-state for menu navigation
///
/// Tracks which menu screen is currently active.
/// This allows menu-specific systems to run conditionally.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, SubStates)]
#[source(GameState = GameState::MainMenu)]
pub enum MenuState {
    /// Main menu screen (default)
    Main,

    /// Game mode selection screen
    ModeSelect,

    /// Credits/about screen
    About,

    /// Piece viewer screen
    ///
    /// Allows viewing and customizing piece materials (color, metallic, roughness, etc.)
    /// with a 3D chess board background showing all pieces.
    PieceViewer,
}

impl Default for MenuState {
    fn default() -> Self {
        Self::Main
    }
}

/// Computed state active during any menu screen
///
/// Allows systems to run during all menu-related states without
/// checking multiple state variants.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct InMenus;

impl ComputedStates for InMenus {
    type SourceStates = GameState;

    fn compute(sources: GameState) -> Option<Self> {
        match sources {
            GameState::MainMenu | GameState::Settings | GameState::MultiplayerMenu => Some(Self),
            _ => None,
        }
    }
}

/// Computed state active during gameplay (including pause)
///
/// Useful for systems that need to run during both active gameplay
/// and pause menu (e.g., rendering the game board).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct InGameplay;

impl ComputedStates for InGameplay {
    type SourceStates = GameState;

    fn compute(sources: GameState) -> Option<Self> {
        match sources {
            GameState::InGame | GameState::Paused | GameState::GameOver => Some(Self),
            _ => None,
        }
    }
}

/// Resource tracking which menu the player navigated from
///
/// Used to return to the correct menu when exiting settings.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct PreviousState {
    pub state: GameState,
}

impl Default for PreviousState {
    fn default() -> Self {
        Self {
            state: GameState::MainMenu,
        }
    }
}

/// Debug helper for logging state transitions
///
/// This system can be toggled on/off with F12 during development to track state changes.
/// It prints the current GameState whenever it's called, useful for debugging state transitions.
pub fn debug_current_gamestate(state: Res<State<GameState>>) {
    info!("[DEBUG] Current State: {:?}", state.get());
}

/// Timer resource for state logging system
///
/// Logs the current game state every 15 seconds for debugging purposes.
#[derive(Resource, Deref, DerefMut)]
pub struct StateLoggerTimer(pub Timer);

impl Default for StateLoggerTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(15.0, TimerMode::Repeating))
    }
}

/// System that logs the current game state every 15 seconds
///
/// Provides comprehensive state information including:
/// - Current GameState
/// - Current MenuState (if in MainMenu)
/// - Computed states (InMenus, InGameplay)
pub fn log_game_state_system(
    state: Res<State<GameState>>,
    menu_state: Option<Res<State<MenuState>>>,
    mut timer: ResMut<StateLoggerTimer>,
    time: Res<Time>,
    persistent_camera: Option<Res<crate::PersistentEguiCamera>>,
    camera_query: Query<Entity, With<bevy_egui::PrimaryEguiContext>>,
) {
    if timer.tick(time.delta()).just_finished() {
        let current_state = state.get();
        let mut state_info = format!("State: {:?}", current_state);

        // Log MenuState if we're in MainMenu
        if *current_state == GameState::MainMenu {
            if let Some(menu_state_res) = menu_state {
                state_info.push_str(&format!(" | Menu: {:?}", menu_state_res.get()));
            }
        }

        // Log computed states (only if relevant)
        if InMenus::compute(*current_state).is_some() {
            state_info.push_str(" | InMenus");
        }
        if InGameplay::compute(*current_state).is_some() {
            state_info.push_str(" | InGameplay");
        }

        info!("[STATE] {}", state_info);

        // Only log camera errors, not status
        if let Some(persistent_camera_res) = persistent_camera {
            if let Some(camera_entity) = persistent_camera_res.entity {
                if camera_query.get(camera_entity).is_err() {
                    error!("[STATE] Camera entity {:?} query failed", camera_entity);
                }
            }
        }
    }
}

/// Validate if a state transition is allowed
///
/// Returns true if the transition is valid according to the game's state machine.
/// Invalid transitions indicate logic errors that should be fixed.
fn is_valid_state_transition(from: GameState, to: GameState) -> bool {
    match (from, to) {
        // MainMenu can transition to Settings or InGame
        (GameState::MainMenu, GameState::Settings) => true,
        (GameState::MainMenu, GameState::InGame) => true,
        (GameState::MainMenu, GameState::MultiplayerMenu) => true,
        (GameState::MultiplayerMenu, GameState::MainMenu) => true,
        (GameState::MultiplayerMenu, GameState::InGame) => true, // Start game from lobby

        // Settings can return to MainMenu
        (GameState::Settings, GameState::MainMenu) => true,

        // InGame can transition to Paused or GameOver
        (GameState::InGame, GameState::Paused) => true,
        (GameState::InGame, GameState::GameOver) => true,

        // Paused can return to InGame or go to MainMenu
        (GameState::Paused, GameState::InGame) => true,
        (GameState::Paused, GameState::MainMenu) => true,
        (GameState::Paused, GameState::Settings) => true, // Allow settings from pause

        // GameOver can go to MainMenu
        (GameState::GameOver, GameState::MainMenu) => true,
        (GameState::GameOver, GameState::InGame) => true, // Allow restart

        // Self-transitions are always valid (no-op)
        (from, to) if from == to => true,

        // All other transitions are invalid
        _ => false,
    }
}

/// System to validate and log state transitions
///
/// Validates state transitions according to the game's state machine and logs
/// errors for invalid transitions. This helps catch logic errors that could
/// cause inconsistent game state.
pub fn validate_and_log_state_transitions(
    mut transition_events: MessageReader<StateTransitionEvent<GameState>>,
) {
    for event in transition_events.read() {
        // Validate and log transition
        match (event.exited, event.entered) {
            (Some(exited), Some(entered)) => {
                let is_valid = is_valid_state_transition(exited, entered);

                if is_valid {
                    info!("[TRANSITION] {:?} -> {:?}", exited, entered);
                } else {
                    error!(
                        "[TRANSITION] INVALID: {:?} -> {:?} (state may be inconsistent)",
                        exited, entered
                    );
                }
            }
            (Some(exited), None) => {
                debug!("[TRANSITION] Exit: {:?}", exited);
            }
            (None, Some(entered)) => {
                debug!("[TRANSITION] Enter: {:?}", entered);
            }
            (None, None) => {
                // Skip empty transitions
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_state_default() {
        let state = GameState::default();
        assert_eq!(state, GameState::MainMenu, "Game should start at main menu");
    }

    #[test]
    fn test_game_state_variants() {
        let menu = GameState::MainMenu;
        let settings = GameState::Settings;
        let game = GameState::InGame;
        let paused = GameState::Paused;
        let over = GameState::GameOver;

        // Ensure all states are distinct
        assert_ne!(menu, settings);
        assert_ne!(settings, game);
        assert_ne!(game, paused);
        assert_ne!(paused, over);
        assert_ne!(over, menu);
    }

    #[test]
    fn test_in_menus_computed_state() {
        // Should be active in interactive menu states
        assert!(InMenus::compute(GameState::MainMenu).is_some());
        assert!(InMenus::compute(GameState::Settings).is_some());

        // Should be inactive in gameplay states
        assert!(InMenus::compute(GameState::InGame).is_none());
        assert!(InMenus::compute(GameState::Paused).is_none());
        assert!(InMenus::compute(GameState::GameOver).is_none());
    }

    #[test]
    fn test_in_gameplay_computed_state() {
        // Should be active in gameplay-related states
        assert!(InGameplay::compute(GameState::InGame).is_some());
        assert!(InGameplay::compute(GameState::Paused).is_some());
        assert!(InGameplay::compute(GameState::GameOver).is_some());

        // Should be inactive in menu states
        assert!(InGameplay::compute(GameState::MainMenu).is_none());
        assert!(InGameplay::compute(GameState::Settings).is_none());
    }

    #[test]
    fn test_previous_state_default() {
        let prev = PreviousState::default();
        assert_eq!(prev.state, GameState::MainMenu);
    }

    #[test]
    fn test_menu_state_default() {
        let menu = MenuState::default();
        assert_eq!(menu, MenuState::Main);
    }
}
