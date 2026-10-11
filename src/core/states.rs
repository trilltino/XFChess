//! The app's state machine and the game mode.
//!
//! ```text
//! MainMenu ──► InGame ──► GameOver ──► MainMenu
//!                ▲  │ ▲       │
//!                └──┘ └───────┘   re-entering InGame (puzzle "Next", rematch)
//! ```
//!
//! - [`GameState`] is the screen. [`MenuState`] is a sub-state of `MainMenu`;
//!   [`InGameplay`] is computed (`InGame` or `GameOver`).
//! - [`GameMode`] says what kind of game is on the board.
//! - Entities live as long as their state: menu entities carry Bevy's
//!   `DespawnOnExit(GameState::MainMenu)`, and the game scene carries
//!   [`game_scene`].

use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, States, Reflect)]
pub enum GameState {
    #[default]
    MainMenu,
    InGame,
    /// The result popup over the final position (the game scene stays).
    GameOver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect, Default)]
pub enum GameMode {
    #[default]
    SinglePlayer,
    MultiplayerLocal,
    MultiplayerCompetitive,
    OnlineMultiplayer,
    Spectator,
    PgnReplay,
    /// A server-verified puzzle: the player moves one colour, the server's replies
    /// arrive as network moves, and the board stays on the player's side.
    Puzzle,
}

/// Which menu flow is open; exists only in `GameState::MainMenu`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, SubStates, Reflect)]
#[source(GameState = GameState::MainMenu)]
pub enum MenuState {
    #[default]
    Main,
    ModeSelect,
    BraidLobby,
    LobbySelection,
    #[cfg(feature = "solana")]
    SolanaLobby,
    Tournaments,
    HostConfig,
    P2PWaiting,
}

/// A game is on screen: `InGame`, or its `GameOver` popup.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct InGameplay;

impl ComputedStates for InGameplay {
    type SourceStates = GameState;

    // Computed states otherwise re-run OnExit/OnEnter on every source transition,
    // even InGame → GameOver; here those schedules mean "left/entered gameplay".
    const ALLOW_SAME_STATE_TRANSITIONS: bool = false;

    fn compute(sources: GameState) -> Option<Self> {
        match sources {
            GameState::InGame | GameState::GameOver => Some(Self),
            GameState::MainMenu => None,
        }
    }
}

/// Marks an entity as part of the game scene (board, pieces, lights, effects).
///
/// The scene outlives `InGame → GameOver`, so the popup shows the final position,
/// and is despawned when the game ends for good (leaving for the menu) or when
/// `InGame` is entered again (puzzle "Next", rematch), which builds a new scene.
/// Bevy evaluates this between the `OnExit` and `OnEnter` schedules, so the new
/// scene is never caught by it.
pub fn game_scene() -> DespawnWhen<GameState> {
    DespawnWhen::new(|transition| {
        matches!(
            transition.exited,
            Some(GameState::InGame | GameState::GameOver)
        ) && transition.entered != Some(GameState::GameOver)
    })
}

pub fn debug_current_gamestate(state: Res<State<GameState>>) {
    debug!("[DEBUG] Current State: {:?}", state.get());
}

/// Logs every `GameState` transition; registered with `on_message`, so it only
/// runs when one happens.
pub fn log_state_transitions(mut transitions: MessageReader<StateTransitionEvent<GameState>>) {
    for transition in transitions.read() {
        match (transition.exited, transition.entered) {
            (Some(GameState::MainMenu), Some(GameState::GameOver)) => {
                error!("[TRANSITION] INVALID: MainMenu -> GameOver (no game to end)");
            }
            (Some(exited), Some(entered)) => info!("[TRANSITION] {exited:?} -> {entered:?}"),
            (None, Some(entered)) => debug!("[TRANSITION] Enter: {entered:?}"),
            (Some(exited), None) => debug!("[TRANSITION] Exit: {exited:?}"),
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    #[test]
    fn game_starts_at_main_menu() {
        assert_eq!(GameState::default(), GameState::MainMenu);
        assert_eq!(MenuState::default(), MenuState::Main);
    }

    #[test]
    fn gameplay_covers_the_board_and_its_result() {
        assert!(InGameplay::compute(GameState::InGame).is_some());
        assert!(InGameplay::compute(GameState::GameOver).is_some());
        assert!(InGameplay::compute(GameState::MainMenu).is_none());
    }

    #[derive(Resource, Default)]
    struct GameplayExits(u32);

    fn app_at(state: GameState) -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .insert_state(state)
            .add_computed_state::<InGameplay>()
            .init_resource::<GameplayExits>()
            .add_systems(OnExit(InGameplay), |mut exits: ResMut<GameplayExits>| {
                exits.0 += 1
            });
        app.update();
        app
    }

    fn go(app: &mut App, state: GameState) {
        app.world_mut().commands().set_state(state);
        app.update();
    }

    fn alive(app: &App, entity: Entity) -> bool {
        app.world().get_entity(entity).is_ok()
    }

    #[test]
    fn scene_survives_the_result_popup_and_ends_with_the_game() {
        let mut app = app_at(GameState::InGame);
        let scene = app.world_mut().spawn(game_scene()).id();

        go(&mut app, GameState::GameOver);
        assert!(alive(&app, scene), "the popup shows the final position");
        assert_eq!(app.world().resource::<GameplayExits>().0, 0);

        go(&mut app, GameState::MainMenu);
        assert!(!alive(&app, scene));
        assert_eq!(app.world().resource::<GameplayExits>().0, 1);
    }

    #[test]
    fn reentering_the_game_rebuilds_the_scene() {
        for from in [GameState::InGame, GameState::GameOver] {
            let mut app = app_at(from);
            let scene = app.world_mut().spawn(game_scene()).id();
            go(&mut app, GameState::InGame);
            assert!(!alive(&app, scene), "re-entering InGame from {from:?}");
            assert_eq!(
                app.world().resource::<GameplayExits>().0,
                0,
                "still in gameplay, so OnExit(InGameplay) must not run"
            );
        }
    }

    #[test]
    fn scene_spawned_on_enter_survives_reentry() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .insert_state(GameState::InGame)
            .add_systems(OnEnter(GameState::InGame), |mut commands: Commands| {
                commands.spawn(game_scene());
            });
        app.update();
        go(&mut app, GameState::InGame);
        let scenes = app
            .world_mut()
            .query::<&DespawnWhen<GameState>>()
            .iter(app.world())
            .count();
        assert_eq!(scenes, 1, "old scene gone, new one kept");
    }
}
