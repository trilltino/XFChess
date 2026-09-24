use bevy::prelude::*;

#[derive(Component)]
pub struct MiniMoveAnimation {
    pub start: Vec3,
    pub end: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

#[derive(Component)]
pub struct MiniFadeOut {
    pub timer: Timer,
    pub initial_scale: f32,
}
