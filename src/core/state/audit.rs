//! Diagnostics around state changes. Despawning itself is Bevy's: entities carry
//! `DespawnOnExit(GameState::MainMenu)` or [`game_scene`](super::game_scene).

use super::GameState;
use bevy::prelude::*;

/// Warns when game pieces survive into the menu, or menu visuals into a game.
/// Run on `state_changed::<GameState>`: leaks can only appear at a transition.
pub fn audit_cross_state_leaks(
    state: Res<State<GameState>>,
    pieces: Query<(), With<crate::rendering::pieces::Piece>>,
    menu_bg: Query<(), With<crate::states::main_menu::menu::MenuBg>>,
) {
    let state = *state.get();
    let piece_count = pieces.iter().count();
    let menu_count = menu_bg.iter().count();

    if piece_count > 0 && state == GameState::MainMenu {
        warn!(
            "[STATE_AUDIT] LEAK: {piece_count} `Piece` entities present in MainMenu — \
             the game scene should have been despawned (see core::game_scene)."
        );
    }
    if menu_count > 0 && state != GameState::MainMenu {
        warn!(
            "[STATE_AUDIT] LEAK: {menu_count} `MenuBg` entities present in {state:?} — \
             menu visuals should only exist on MainMenu."
        );
    }
}

/// Periodic entity count for debugging leaks (debug log level).
pub fn periodic_entity_audit(
    all_entities: Query<()>,
    scene: Query<(), With<DespawnWhen<GameState>>>,
    menu: Query<(), With<DespawnOnExit<GameState>>>,
    state: Res<State<GameState>>,
) {
    debug!(
        "[STATE_AUDIT] {:?} | {} entities | {} game scene | {} menu",
        state.get(),
        all_entities.iter().count(),
        scene.iter().count(),
        menu.iter().count(),
    );
}
