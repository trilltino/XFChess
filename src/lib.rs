/// XFChess library module for decentralized chess on Solana

pub mod assets;
pub mod core;
pub mod engine;
pub mod game;
pub mod input;
pub mod multiplayer;
pub mod presentation;
pub mod rendering;
#[cfg(feature = "solana")]
pub mod solana;
pub mod states;
pub mod ui;
pub mod xf_animate;

use bevy::prelude::*;
use bevy::asset::AssetMetaCheck;
use bevy::audio::{AudioPlugin, Volume};
use bevy::log::LogPlugin;
use bevy_egui::EguiPlugin;
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use core::persistent_camera::PersistentEguiCamera;

/// Player color option
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum PlayerColor {
    White,
    Black,
}

/// Game configuration from CLI arguments and environment variables.
/// This struct serves as both the CLI parser and the Bevy resource.
#[derive(Parser, Resource, Debug, Clone)]
#[command(name = "xfchess")]
#[command(about = "XFChess - Decentralized Chess with Ephemeral Rollups")]
#[command(version = "0.1.0")]
pub struct GameConfig {
    /// Optional game ID for joining an existing game
    #[arg(long)]
    pub game_id: Option<u64>,

    /// Player color (White or Black)
    #[arg(long, value_enum)]
    pub player_color: Option<PlayerColor>,

    /// Solana RPC endpoint URL
    #[arg(long, default_value = "https://api.devnet.solana.com", env = "XFCHESS_RPC_URL")]
    pub rpc_url: String,

    /// Session key (base58 encoded) for signing rollups
    #[arg(long, env = "XFCHESS_SESSION_KEY")]
    pub session_key: Option<String>,

    /// Session public key
    #[arg(long, env = "XFCHESS_SESSION_PUBKEY")]
    pub session_pubkey: Option<String>,

    /// P2P network port
    #[arg(long, default_value = "5001", env = "XFCHESS_P2P_PORT")]
    pub p2p_port: u16,

    /// Bootstrap node ID (for Player 2 to connect to Player 1)
    #[arg(long, env = "XFCHESS_BOOTSTRAP_NODE")]
    pub bootstrap_node: Option<String>,

    /// Game PDA address
    #[arg(long, env = "XFCHESS_GAME_PDA")]
    pub game_pda: Option<String>,

    /// Wager amount in SOL
    #[arg(long, env = "XFCHESS_WAGER_AMOUNT")]
    pub wager_amount: Option<f64>,

    /// Enable transaction debugger / debug mode
    #[arg(long)]
    pub debug: bool,

    /// Log file path
    #[arg(long, default_value = "rollup_debug.log")]
    pub log_file: String,

    /// AI difficulty (1-5)
    #[arg(long, env = "XFCHESS_AI_DIFFICULTY")]
    pub ai_difficulty: Option<u8>,

    /// AI side (White or Black)
    #[arg(long, env = "XFCHESS_AI_SIDE")]
    pub ai_side: Option<PlayerColor>,

    /// Session config JSON file path
    #[arg(long)]
    pub session_config: Option<PathBuf>,

    /// Subcommand for CLI-only tools
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Tournament administrator controls
    Tournament {
        #[command(subcommand)]
        action: TournamentCommand,
    },
    /// Run the transaction debugger (integrated view)
    Debug {
        /// Game ID to monitor
        #[arg(long)]
        game_id: u64,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum TournamentCommand {
    /// Create a new tournament on-chain
    Create {
        #[arg(long, default_value = "XFChess Cup")]
        name: String,
        #[arg(long, default_value = "0.05")]
        entry_fee: f64,
    },
    /// List active tournaments
    List,
    /// Start tournament bracket
    Start {
        #[arg(long)]
        id: u64,
    },
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
            ai_difficulty: None,
            ai_side: None,
            session_config: None,
            command: None,
        }
    }
}

impl GameConfig {
    /// Load session config from JSON file if specified
    pub fn load_session_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref path) = self.session_config {
            info!("Loading session config from: {}", path.display());
            let contents = std::fs::read_to_string(path)?;
            let session: SessionConfigFile = serde_json::from_str(&contents)?;

            self.game_id = Some(session.game_id.parse()?);
            self.player_color = Some(match session.player_color.to_lowercase().as_str() {
                "white" => PlayerColor::White,
                "black" => PlayerColor::Black,
                _ => PlayerColor::White,
            });
            self.session_key = Some(session.session_key);
            self.session_pubkey = Some(session.session_pubkey);
            self.rpc_url = session.rpc_url;
            self.game_pda = Some(session.game_pda);
            self.wager_amount = Some(session.wager_amount);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct SessionConfigFile {
    pub game_id: String,
    pub player_color: String,
    pub session_key: String,
    pub session_pubkey: String,
    pub rpc_url: String,
    pub game_pda: String,
    pub wager_amount: f64,
}

/// Builds the Bevy application with all plugins and configuration
pub fn build_app(game_config: GameConfig) -> App {
    let mut app = App::new();
    
    // Configure AI if requested
    if let Some(diff_val) = game_config.ai_difficulty {
        use crate::game::ai::resource::{ChessAIResource, GameMode, AIDifficulty};
        use crate::rendering::pieces::PieceColor;
        
        let difficulty = AIDifficulty::from_u8(diff_val);
        let ai_color = match game_config.ai_side.unwrap_or(PlayerColor::Black) {
            PlayerColor::White => PieceColor::White,
            PlayerColor::Black => PieceColor::Black,
        };
        
        info!("[AI] Initializing VS Computer Match: {} (Side: {:?})", 
            difficulty.description(), ai_color);
            
        app.insert_resource(ChessAIResource {
            mode: GameMode::VsAI { ai_color },
            difficulty,
        });
    }

    app.insert_resource(game_config.clone())
        .init_resource::<PersistentEguiCamera>()
        .insert_resource(bevy_egui::EguiGlobalSettings {
            auto_create_primary_context: false,
            ..default()
        })
        .add_systems(Startup, core::persistent_camera::setup_persistent_egui_camera);

    // Add core plugins
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                #[cfg(not(target_arch = "wasm32"))]
                file_path: {
                    let cwd_assets = std::path::PathBuf::from("assets");
                    if cwd_assets.exists() && cwd_assets.is_dir() {
                        "assets".to_string()
                    } else {
                        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("assets")
                            .to_string_lossy()
                            .into_owned()
                    }
                },
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "XFChess".to_string(),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: false,
                    ..default()
                }),
                ..default()
            })
            .set(AudioPlugin {
                global_volume: GlobalVolume {
                    volume: Volume::Linear(0.3),
                },
                ..default()
            })
            .set(LogPlugin {
                filter: if cfg!(debug_assertions) {
                    "info,wgpu_core=warn,wgpu_hal=warn,xfchess=info,bevy_gltf=error,bevy_image=error".to_string()
                } else {
                    "error".to_string()
                },
                ..default()
            }),
    )
    .add_plugins(EguiPlugin::default());

    // Add custom plugins
    app.add_plugins((
        core::CorePlugin,
        game::GamePlugin,
        rendering::RenderingPlugin,
        ui::UiPlugin,
        input::InputPlugin,
        presentation::PresentationPlugin,
    ))
    .add_plugins((
        states::main_menu::MainMenuPlugin,
        states::game_over::GameOverPlugin,
        states::pause::PausePlugin,
        states::piece_viewer::PieceViewerPlugin,
        xf_animate::XfAnimatePlugin,
    ))
    .add_plugins(multiplayer::MultiplayerPlugin);

    #[cfg(feature = "solana")]
    app.add_plugins(solana::SolanaPlugin);

    app
}
