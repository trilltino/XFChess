use bevy::prelude::*;

pub const SQUARE_SIZE: f32 = 1.0;
pub const BOARD_HALF: f32 = 4.0;

pub fn square_world(file: u8, rank: u8) -> Vec3 {
    Vec3::new(
        file as f32 * SQUARE_SIZE - BOARD_HALF + SQUARE_SIZE * 0.5,
        0.0,
        BOARD_HALF - (rank as f32 * SQUARE_SIZE) - SQUARE_SIZE * 0.5,
    )
}
