//! Game state definitions and transitions
//!
//! Implements Bevy 0.17's state management system for XFChess, providing clean separation
//! between the launch menu and active gameplay.
//!
//! # State Flow
//!
//! ```text
//! [LaunchMenu] --Start Game--> [Multiplayer] --Exit--> [LaunchMenu]
//! ```
//!
//! # Bevy 0.17 Features
//!
//! - `States` derive macro for automatic state management
//! - `ComputedStates` for conditional system execution
//! - State transitions via `NextState<GameState>`
//!
//! # Reference
//!
//! Pattern based on `reference/bevy/examples/state/states.rs` which demonstrates
//! the modern Bevy 0.17 state system replacing the older `State<T>` resource pattern.
//!
//! # Example Usage
//!
//! ```rust
//! use bevy::prelude::*;
//! # use xfchess::core::GameState;
//!
//! fn setup_game(mut next_state: ResMut<NextState<GameState>>) {
//!     // Transition from launch menu to active gameplay
//!     next_state.set(GameState::Multiplayer);
//! }
//!
//! // System that only runs during gameplay
//! fn gameplay_system() {
//!     println!("Game is active!");
//! }
//!
//! // Add to app with state-conditional execution
//! # fn example_app() {
//! App::new()
//!     .init_state::<GameState>()
//!     .add_systems(Update, gameplay_system.run_if(in_state(GameState::Multiplayer)));
//! # }
//! ```

use bevy::prelude::*;

/// Primary game state controlling major application modes
#[derive(Clone, Copy, Resource, PartialEq, Eq, Hash, Debug, Default, States)]
pub enum GameState {
    /// Main menu state - shows launch UI
    #[default]
    LaunchMenu,
    /// Active game state - chess gameplay
    Multiplayer,
}

/// Computed state active only during launch menu
///
/// Allows systems to run conditionally with `.run_if(in_state(LaunchMenu))`
/// without directly checking the GameState resource.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct LaunchMenu;

impl ComputedStates for LaunchMenu {
    type SourceStates = GameState;

    /// Computes whether the LaunchMenu state is active based on GameState
    ///
    /// # Returns
    /// - `Some(LaunchMenu)` if GameState is LaunchMenu (menu UI systems should run)
    /// - `None` if GameState is Multiplayer (menu UI systems should not run)
    ///
    /// This allows Bevy to automatically enable/disable menu-specific systems
    /// without manual state checking in every system.
    fn compute(sources: GameState) -> Option<Self> {
        match sources {
            GameState::LaunchMenu => Some(Self), // Menu systems active
            _ => None,                            // Menu systems inactive
        }
    }
}

/// Debug helper for logging state transitions
///
/// This system can be toggled on/off with F12 during development to track state changes.
/// It prints the current GameState whenever it's called, useful for debugging state transitions.
pub fn debug_current_gamestate(state: Res<State<GameState>>) {
    println!("Current State: {:?}", state.get());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_state_default() {
        //! Verifies GameState defaults to LaunchMenu
        //!
        //! The game should always start in the launch menu state,
        //! not in active gameplay.
        let state = GameState::default();
        assert_eq!(state, GameState::LaunchMenu);
    }

    #[test]
    fn test_game_state_variants() {
        //! Tests all GameState variants are distinct
        //!
        //! Ensures the two states are properly differentiated.
        let launch = GameState::LaunchMenu;
        let game = GameState::Multiplayer;

        assert_ne!(launch, game);
        assert_eq!(launch, GameState::LaunchMenu);
        assert_eq!(game, GameState::Multiplayer);
    }

    #[test]
    fn test_game_state_clone() {
        //! Verifies GameState implements Clone correctly
        //!
        //! State cloning is essential for Bevy's state management system.
        let state = GameState::Multiplayer;
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_game_state_copy() {
        //! Verifies GameState implements Copy semantics
        //!
        //! Copy is required for efficient state comparisons.
        let state = GameState::LaunchMenu;
        let copied = state;
        assert_eq!(state, copied); // Original still valid (Copy, not Move)
    }

    #[test]
    fn test_launch_menu_computed_state_active() {
        //! Tests LaunchMenu computed state returns Some when in LaunchMenu
        //!
        //! The computed state should be active (Some) only when the
        //! source GameState is LaunchMenu.
        let result = LaunchMenu::compute(GameState::LaunchMenu);
        assert!(result.is_some());
    }

    #[test]
    fn test_launch_menu_computed_state_inactive() {
        //! Tests LaunchMenu computed state returns None when in Multiplayer
        //!
        //! The computed state should be inactive (None) when GameState
        //! is anything other than LaunchMenu.
        let result = LaunchMenu::compute(GameState::Multiplayer);
        assert!(result.is_none());
    }

    #[test]
    fn test_launch_menu_clone() {
        //! Verifies LaunchMenu computed state can be cloned
        let menu = LaunchMenu;
        let cloned = menu.clone();
        assert_eq!(menu, cloned);
    }

    #[test]
    fn test_launch_menu_copy() {
        //! Verifies LaunchMenu implements Copy semantics
        let menu = LaunchMenu;
        let copied = menu;
        assert_eq!(menu, copied);
    }

    #[test]
    fn test_launch_menu_equality() {
        //! Tests LaunchMenu instances are equal (unit struct)
        let menu1 = LaunchMenu;
        let menu2 = LaunchMenu;
        assert_eq!(menu1, menu2);
    }

    #[test]
    fn test_launch_menu_hash_consistency() {
        //! Verifies LaunchMenu hashes consistently
        //!
        //! Required for use in HashMap/HashSet data structures.
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let menu1 = LaunchMenu;
        let menu2 = LaunchMenu;

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();

        menu1.hash(&mut hasher1);
        menu2.hash(&mut hasher2);

        assert_eq!(hasher1.finish(), hasher2.finish());
    }
}
