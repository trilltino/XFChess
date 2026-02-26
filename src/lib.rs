pub mod assets;
pub mod core;
pub mod engine;
pub mod game;
pub mod input;
pub mod multiplayer;
pub mod persistent_camera;
pub mod presentation;
pub mod rendering;
pub mod singleplayer;
#[cfg(feature = "solana")]
pub mod solana;
pub mod states;
pub mod ui;

pub use persistent_camera::PersistentEguiCamera;

/// XFChess program ID on Solana
/// TODO: Replace with actual deployed program ID
pub const PROGRAM_ID: &str = "XFChessGame1111111111111111111111111111111";
