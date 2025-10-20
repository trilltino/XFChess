//! Integration tests for XFChess core state management
//!
//! Tests the state management system in a realistic Bevy application context,
//! verifying that state transitions work correctly and systems execute only
//! in their designated states.

use bevy::prelude::*;
use xfchess::core::{GameState, LaunchMenu, debug_current_gamestate};

/// Helper struct to track system executions during tests
#[derive(Resource, Default, Debug)]
struct SystemExecutionTracker {
    launch_menu_executions: u32,
    multiplayer_executions: u32,
}

/// Test system that runs only in LaunchMenu state
fn track_launch_menu_execution(mut tracker: ResMut<SystemExecutionTracker>) {
    tracker.launch_menu_executions += 1;
}

/// Test system that runs only in Multiplayer state
fn track_multiplayer_execution(mut tracker: ResMut<SystemExecutionTracker>) {
    tracker.multiplayer_executions += 1;
}

#[test]
fn test_initial_state_is_launch_menu() {
    //! Verifies that a new app starts in the LaunchMenu state
    //!
    //! This ensures users see the launch menu when the game first starts,
    //! not the active gameplay screen.

    let mut app = App::new();
    app.init_state::<GameState>();

    // Run one update cycle
    app.update();

    // Extract the state and verify it's LaunchMenu
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::LaunchMenu);
}

#[test]
fn test_state_transition_to_multiplayer() {
    //! Tests transitioning from LaunchMenu to Multiplayer state
    //!
    //! Simulates a user clicking "Start Game" in the menu, which should
    //! transition the app to the active gameplay state.

    let mut app = App::new();
    app.init_state::<GameState>();

    // Trigger state transition
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Multiplayer);

    // Update to apply the state change
    app.update();

    // Verify state changed
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::Multiplayer);
}

#[test]
fn test_state_transition_back_to_launch_menu() {
    //! Tests round-trip state transition: LaunchMenu -> Multiplayer -> LaunchMenu
    //!
    //! Simulates starting a game and then returning to the main menu
    //! (e.g., pressing ESC or finishing a game).

    let mut app = App::new();
    app.init_state::<GameState>();

    // Start in LaunchMenu (default)
    app.update();
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::LaunchMenu);

    // Transition to Multiplayer
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Multiplayer);
    app.update();
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::Multiplayer);

    // Transition back to LaunchMenu
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::LaunchMenu);
    app.update();
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::LaunchMenu);
}

#[test]
fn test_systems_run_conditionally_based_on_state() {
    //! Verifies that systems with `in_state()` run conditions execute only in correct states
    //!
    //! This ensures menu systems don't run during gameplay and vice versa,
    //! preventing bugs like menu UI appearing during a game.

    let mut app = App::new();
    app.init_state::<GameState>();
    app.init_resource::<SystemExecutionTracker>();

    // Add state-conditional systems
    app.add_systems(
        Update,
        track_launch_menu_execution.run_if(in_state(GameState::LaunchMenu)),
    );
    app.add_systems(
        Update,
        track_multiplayer_execution.run_if(in_state(GameState::Multiplayer)),
    );

    // Initially in LaunchMenu - only launch menu system should run
    app.update();
    {
        let tracker = app.world().resource::<SystemExecutionTracker>();
        assert_eq!(tracker.launch_menu_executions, 1);
        assert_eq!(tracker.multiplayer_executions, 0);
    }

    // Transition to Multiplayer
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Multiplayer);
    app.update();

    // Now only multiplayer system should have run
    {
        let tracker = app.world().resource::<SystemExecutionTracker>();
        assert_eq!(tracker.launch_menu_executions, 1); // Unchanged
        assert_eq!(tracker.multiplayer_executions, 1); // Incremented
    }

    // Update again in Multiplayer state
    app.update();
    {
        let tracker = app.world().resource::<SystemExecutionTracker>();
        assert_eq!(tracker.launch_menu_executions, 1); // Still unchanged
        assert_eq!(tracker.multiplayer_executions, 2); // Incremented again
    }
}

#[test]
fn test_computed_state_launch_menu() {
    //! Tests that the LaunchMenu computed state activates/deactivates correctly
    //!
    //! Computed states allow more granular control over system execution
    //! without checking the main state repeatedly.

    let mut app = App::new();
    app.init_state::<GameState>();
    app.add_computed_state::<LaunchMenu>();
    app.init_resource::<SystemExecutionTracker>();

    // Add system that runs only when LaunchMenu computed state is active
    app.add_systems(
        Update,
        track_launch_menu_execution.run_if(in_state(LaunchMenu)),
    );

    // Start in LaunchMenu - system should run
    app.update();
    {
        let tracker = app.world().resource::<SystemExecutionTracker>();
        assert_eq!(tracker.launch_menu_executions, 1);
    }

    // Transition to Multiplayer - LaunchMenu computed state becomes inactive
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Multiplayer);
    app.update();

    // System should not have run again
    {
        let tracker = app.world().resource::<SystemExecutionTracker>();
        assert_eq!(tracker.launch_menu_executions, 1); // Unchanged
    }
}

#[test]
fn test_multiple_state_transitions() {
    //! Stress test: Multiple rapid state transitions should work correctly
    //!
    //! Simulates edge cases like rapid menu navigation or game restarts.

    let mut app = App::new();
    app.init_state::<GameState>();

    // Perform multiple transitions
    for i in 0..10 {
        let target_state = if i % 2 == 0 {
            GameState::Multiplayer
        } else {
            GameState::LaunchMenu
        };

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(target_state);
        app.update();

        let state = app.world().resource::<State<GameState>>();
        assert_eq!(*state.get(), target_state);
    }
}

#[test]
fn test_debug_current_gamestate_system() {
    //! Verifies the debug_current_gamestate system doesn't panic
    //!
    //! While this system just prints debug info, we ensure it can
    //! safely access the state resource.

    let mut app = App::new();
    app.init_state::<GameState>();
    app.add_systems(Update, debug_current_gamestate);

    // Should not panic
    app.update();
    app.update();

    // Verify we can still access state after debug system runs
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::LaunchMenu);
}

#[test]
fn test_state_persistence_across_updates() {
    //! Verifies state remains stable across multiple update cycles
    //!
    //! Ensures states don't spontaneously change without explicit transitions.

    let mut app = App::new();
    app.init_state::<GameState>();

    // Set to Multiplayer
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Multiplayer);
    app.update();

    // Run many updates without changing state
    for _ in 0..100 {
        app.update();
        let state = app.world().resource::<State<GameState>>();
        assert_eq!(*state.get(), GameState::Multiplayer);
    }
}

#[test]
fn test_state_is_clonable() {
    //! Tests that GameState can be cloned (required for Bevy internals)
    //!
    //! Bevy's state system relies on Clone for efficient state management.

    let state1 = GameState::LaunchMenu;
    let state2 = state1.clone();
    assert_eq!(state1, state2);

    let state3 = GameState::Multiplayer;
    let state4 = state3.clone();
    assert_eq!(state3, state4);
}

#[test]
fn test_state_is_copyable() {
    //! Tests that GameState implements Copy (more efficient than Clone)
    //!
    //! Copy allows Bevy to pass states by value without heap allocations.

    let state1 = GameState::LaunchMenu;
    let state2 = state1; // Copy, not move
    assert_eq!(state1, state2);
    // state1 is still accessible (Copy, not Move)
    assert_eq!(state1, GameState::LaunchMenu);
}

#[test]
fn test_state_debug_format() {
    //! Verifies GameState has useful Debug output
    //!
    //! Good debug formatting helps with logging and troubleshooting.

    let debug_str = format!("{:?}", GameState::LaunchMenu);
    assert!(debug_str.contains("LaunchMenu"));

    let debug_str = format!("{:?}", GameState::Multiplayer);
    assert!(debug_str.contains("Multiplayer"));
}
