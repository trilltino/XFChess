//! Game state system for XFChess.

use bevy::prelude::*;
// use bevy::ecs::event::EventReader; // EventReader is in prelude

/// Primary game state controlling major application modes.
#[derive(Clone, Copy, Resource, PartialEq, Eq, Hash, Debug, Default, States, Reflect)]
pub enum GameState {
    /// Authentication state (Login/Register)
    #[cfg_attr(target_arch = "wasm32", default)]
    Auth,

    /// Main menu state (starting state)
    #[cfg_attr(not(target_arch = "wasm32"), default)]
    MainMenu,

    /// Active gameplay state
    InGame,

    /// Paused game state
    Paused,

    /// Game over state
    GameOver,

    /// Multiplayer Lobby Menu
    MultiplayerMenu,

    /// Matching state (Connecting/Handshake)
    Matching,
}

/// Define an enum for game modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect)]
pub enum GameMode {
    SinglePlayer,
    MultiplayerLocal,
    MultiplayerCompetitive,
    BraidMultiplayer,
}

impl Default for GameMode {
    fn default() -> Self {
        Self::SinglePlayer
    }
}

/// Component marking entities to be despawned when exiting a specific state.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DespawnOnExit<T>(pub T)
where
    T: States + Copy;

/// Sub-state for menu navigation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, SubStates)]
#[source(GameState = GameState::MainMenu)]
pub enum MenuState {
    /// Main menu screen (default)
    Main,

    /// Game mode selection screen
    ModeSelect,

    /// Braid Multiplayer Lobby
    BraidLobby,

    /// Solana Multiplayer Lobby
    #[cfg(feature = "solana")]
    SolanaLobby,

    /// Credits/about screen
    About,

    /// Piece viewer screen
    PieceViewer,
}

impl Default for MenuState {
    fn default() -> Self {
        Self::Main
    }
}

/// Computed state active during any menu screen.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct InMenus;

impl ComputedStates for InMenus {
    type SourceStates = GameState;

    fn compute(sources: GameState) -> Option<Self> {
        match sources {
            GameState::MainMenu | GameState::MultiplayerMenu => Some(Self),
            _ => None,
        }
    }
}

/// Computed state active during gameplay (including pause).
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

/// Resource tracking which menu the player navigated from.
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

/// Debug helper for logging state transitions.
pub fn debug_current_gamestate(state: Res<State<GameState>>) {
    info!("[DEBUG] Current State: {:?}", state.get());
}

/// Timer resource for state logging system.
#[derive(Resource, Deref, DerefMut)]
pub struct StateLoggerTimer(pub Timer);

impl Default for StateLoggerTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(15.0, TimerMode::Repeating))
    }
}

/// System that logs the current game state every 15 seconds.
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

/// Validate if a state transition is allowed.
fn is_valid_state_transition(from: GameState, to: GameState) -> bool {
    match (from, to) {
        // MainMenu can transition to InGame
        (GameState::MainMenu, GameState::InGame) => true,
        (GameState::MainMenu, GameState::MultiplayerMenu) => true,
        (GameState::MainMenu, GameState::Auth) => true, // Allowed for FORCE_AUTH override
        (GameState::MultiplayerMenu, GameState::MainMenu) => true,
        (GameState::MultiplayerMenu, GameState::InGame) => true, // Start game from lobby

        // InGame can transition to Paused or GameOver
        (GameState::InGame, GameState::Paused) => true,
        (GameState::InGame, GameState::GameOver) => true,

        // Paused can return to InGame or go to MainMenu
        (GameState::Paused, GameState::InGame) => true,
        (GameState::Paused, GameState::MainMenu) => true,

        // GameOver can go to MainMenu
        (GameState::GameOver, GameState::MainMenu) => true,
        (GameState::GameOver, GameState::InGame) => true, // Allow restart

        // Self-transitions are always valid (no-op)
        (from, to) if from == to => true,

        // All other transitions are invalid
        _ => false,
    }
}

/// System to validate and log state transitions.
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
        #[cfg(target_arch = "wasm32")]
        assert_eq!(state, GameState::Auth, "Game should start at Auth on Web");
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(
            state,
            GameState::MainMenu,
            "Game should start at MainMenu on Native"
        );
    }

    #[test]
    fn test_game_state_variants() {
        let menu = GameState::MainMenu;
        let game = GameState::InGame;
        let paused = GameState::Paused;
        let over = GameState::GameOver;

        // Ensure all states are distinct
        assert_ne!(menu, game);
        assert_ne!(game, paused);
        assert_ne!(paused, over);
        assert_ne!(over, menu);
    }

    #[test]
    fn test_in_menus_computed_state() {
        // Should be active in interactive menu states
        assert!(InMenus::compute(GameState::MainMenu).is_some());

        // Should be inactive in gameplay states
        assert!(InMenus::compute(GameState::InGame).is_none());
        assert!(InMenus::compute(GameState::Paused).is_none());
        assert!(InGameplay::compute(GameState::GameOver).is_some());

        // Should be inactive in menu states
        assert!(InGameplay::compute(GameState::MainMenu).is_none());
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
