//! Shared chess-piece movement. [`curve`] and [`timeline`] are pure and used by
//! every board (game, replay, menu showcase, LEARN); [`gameplay`] attaches them
//! to live-game piece entities for the 2D and 3D views.

pub mod curve;
pub mod gameplay;
pub mod timeline;

#[cfg(test)]
mod tests;

pub use curve::{MotionSample, MotionStyle, GAMEPLAY_MOVE_SECONDS};
pub use gameplay::{
    advance_piece_motions, board_to_world, present_piece_motion_3d, retire_board_visuals,
    square_to_board, BoardEpoch, PieceMotion, StartPieceMotion,
};
pub use timeline::{MoveTransaction, VisualTransactionStatus};
