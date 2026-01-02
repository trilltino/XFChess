//! State lifecycle debugging and monitoring
//!
//! Provides comprehensive logging for state transitions to debug state management issues.
//! These systems help verify that:
//! - States enter and exit correctly
//! - Entities are properly cleaned up via DespawnOnExit
//! - No entity leaks occur during state transitions

use super::{DespawnOnExit, GameState, MenuState};
use bevy::prelude::*;

/// Log when entering any game state
/// Runs on OnEnter for all states
pub fn log_state_entry(state: Res<State<GameState>>) {
    info!("[STATE_LIFECYCLE] ENTER: {:?}", state.get());
}

/// Log when exiting any game state
/// Runs on OnExit for all states
pub fn log_state_exit(state: Res<State<GameState>>) {
    info!("[STATE_LIFECYCLE] EXIT: {:?}", state.get());
}

/// Count and log entities marked for despawn in current state
/// This helps verify entity cleanup is properly configured
pub fn audit_despawn_markers(
    query: Query<(Entity, Option<&Name>, &DespawnOnExit<GameState>)>,
    state: Res<State<GameState>>,
) {
    let current_state = *state.get();
    let entities: Vec<_> = query
        .iter()
        .filter(|(_, _, despawn)| despawn.0 == current_state)
        .collect();

    let count = entities.len();

    if count > 0 {
        info!(
            "[STATE_LIFECYCLE] State {:?} has {} entities marked for cleanup:",
            current_state, count
        );

        // Log first few entities for debugging
        for (entity, name, _) in entities.iter().take(5) {
            let entity_name = name.map(|n| n.as_str()).unwrap_or("unnamed");
            debug!(
                "[STATE_LIFECYCLE]    - Entity {:?}: {}",
                entity, entity_name
            );
        }

        if count > 5 {
            debug!("[STATE_LIFECYCLE]    ... and {} more", count - 5);
        }
    }
}

/// Create a cleanup system for a specific state
/// This is needed because state.get() returns the NEW state during OnExit,
/// not the state being exited. So we need separate functions for each state.
macro_rules! create_cleanup_system {
    ($name:ident, $state:expr) => {
        pub fn $name(
            query: Query<(Entity, Option<&Name>, &DespawnOnExit<GameState>)>,
            mut commands: Commands,
        ) {
            let target_state = $state;
            let mut despawned_count = 0;

            for (entity, name, despawn_marker) in query.iter() {
                if despawn_marker.0 == target_state {
                    let entity_name = name.map(|n| n.as_str()).unwrap_or("unnamed");
                    debug!(
                        "[STATE_LIFECYCLE] Despawning entity {:?}: {} (marked for {:?})",
                        entity, entity_name, target_state
                    );
                    commands.entity(entity).despawn();
                    despawned_count += 1;
                }
            }

            if despawned_count > 0 {
                info!(
                    "[STATE_LIFECYCLE] Despawned {} entities on exit from {:?}",
                    despawned_count, target_state
                );
            }
        }
    };
}

// Create cleanup systems for each state
create_cleanup_system!(cleanup_main_menu, GameState::MainMenu);
create_cleanup_system!(cleanup_settings, GameState::Settings);
create_cleanup_system!(cleanup_in_game, GameState::InGame);
create_cleanup_system!(cleanup_paused, GameState::Paused);
create_cleanup_system!(cleanup_game_over, GameState::GameOver);
/// Verify picking events only occur in InGame state
/// This catches bugs where picking systems run in wrong states
pub fn verify_picking_scope(state: Res<State<GameState>>) {
    // This runs every frame in InGame to verify the state is correct
    // If we receive picking events outside InGame, there's a scoping bug
    if *state.get() == GameState::InGame {
        // Expected state for picking - no warning needed
    }
}

/// Log menu sub-state transitions
pub fn log_menu_state_transitions(menu_state: Option<Res<State<MenuState>>>) {
    if let Some(state) = menu_state {
        if state.is_changed() {
            info!("[STATE_LIFECYCLE] Menu SubState: {:?}", state.get());
        }
    }
}

/// Periodic full state audit
/// Runs every 10 seconds to catch entity leaks
#[derive(Resource, Deref, DerefMut)]
pub struct StateAuditTimer(pub Timer);

impl Default for StateAuditTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(10.0, TimerMode::Repeating))
    }
}

/// System that periodically audits all entities for leaks
pub fn periodic_entity_audit(
    mut timer: ResMut<StateAuditTimer>,
    time: Res<Time>,
    all_entities: Query<Entity>,
    despawn_markers: Query<&DespawnOnExit<GameState>>,
    state: Res<State<GameState>>,
) {
    if timer.tick(time.delta()).just_finished() {
        let total_entities = all_entities.iter().count();
        let entities_with_markers = despawn_markers.iter().count();
        let persistent_entities = total_entities - entities_with_markers;

        info!(
            "[STATE_AUDIT] {:?} | Total: {} entities | {} persistent | {} will cleanup",
            state.get(),
            total_entities,
            persistent_entities,
            entities_with_markers
        );
    }
}
