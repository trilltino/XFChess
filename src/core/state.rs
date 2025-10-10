use bevy::prelude::*;

#[derive(Clone, Copy, Resource, PartialEq, Eq, Hash, Debug, Default, States)]
pub enum GameState {
    #[default]
    LaunchMenu,
    Multiplayer,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct LaunchMenu;

impl ComputedStates for LaunchMenu {
    type SourceStates = GameState;

    fn compute(sources: GameState) -> Option<Self> {
        match sources {
            GameState::LaunchMenu { .. } => Some(Self),
            _ => None,
        }
    }
}

#[allow(dead_code)] // TODO: Useful for debugging state transitions
pub fn debug_current_gamestate(state: Res<State<GameState>>) {
    println!("current State: {:?}", state.get());
}
