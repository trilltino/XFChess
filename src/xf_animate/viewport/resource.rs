use bevy::prelude::*;

pub const MINI_LAYER: usize = 8;

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LearnViewportRect {
    pub rect_px: Option<URect>,
}
