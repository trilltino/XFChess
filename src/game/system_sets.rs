use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum GameSystems {
    Input,

    Validation,

    Execution,

    Visual,
}
