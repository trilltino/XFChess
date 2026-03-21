pub mod assets;
pub mod cli;
pub mod core;
pub mod engine;
pub mod opera_game_metadata;
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

pub use cli::PlayerColor;
pub use persistent_camera::PersistentEguiCamera;

use bevy::prelude::*;

/// Game configuration from CLI arguments
#[derive(Resource, Debug)]
pub struct GameConfig {
    pub game_id: Option<u64>,
    pub player_color: Option<PlayerColor>,
    pub rpc_url: String,
    pub session_key: Option<String>,
    pub session_pubkey: Option<String>,
    pub p2p_port: u16,
    pub bootstrap_node: Option<String>,
    pub game_pda: Option<String>,
    pub wager_amount: Option<f64>,
    pub debug: bool,
    pub log_file: String,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            game_id: None,
            player_color: None,
            rpc_url: "https://api.devnet.solana.com".to_string(),
            session_key: None,
            session_pubkey: None,
            p2p_port: 5001,
            bootstrap_node: None,
            game_pda: None,
            wager_amount: None,
            debug: false,
            log_file: "rollup_debug.log".to_string(),
        }
    }
}

/// XFChess program ID on Solana
/// TODO: Replace with actual deployed program ID
pub const PROGRAM_ID: &str = "XFChessGame1111111111111111111111111111111";
