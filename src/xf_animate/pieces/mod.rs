mod assets;
mod components;
mod setup;
mod spawn;

pub use assets::{MiniAssets, MiniMeshes};
pub use components::MiniPiece;
pub use setup::{spawn_mini_pieces, spawn_starting_position};
pub use spawn::{spawn_mini_piece, PIECE_MESH_SCALE};
