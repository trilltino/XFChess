use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Resource, Default)]
#[reflect(Resource)]
pub enum ViewMode {
    #[default]
    Standard,
    TempleOS,
}
