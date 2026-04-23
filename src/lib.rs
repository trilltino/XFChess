/// XFChess library module for decentralized chess on Solana
pub mod assets;
pub mod cli;
pub mod core;
pub mod engine;
pub mod game;
pub mod input;
pub mod multiplayer;
pub mod presentation;
pub mod rendering;
pub mod singleplayer;
#[cfg(feature = "solana")]
pub mod solana;
pub mod states;
pub mod ui;
pub mod xf_animate;

pub use cli::{Cli, PlayerColor};
pub use core::persistent_camera::PersistentEguiCamera;

use bevy::prelude::*;
use bevy::asset::AssetMetaCheck;
use bevy::audio::{AudioPlugin, Volume};
use bevy::log::LogPlugin;
use bevy_egui::EguiPlugin;

/// Game configuration from CLI arguments and environment variables
#[derive(Resource, Debug, Clone)]
pub struct GameConfig {
    /// Optional game ID for joining an existing game
    pub game_id: Option<u64>,
    /// Player color (White or Black)
    pub player_color: Option<PlayerColor>,
    /// Solana RPC endpoint URL
    pub rpc_url: String,
    /// Session key for delegation
    pub session_key: Option<String>,
    /// Session public key
    pub session_pubkey: Option<String>,
    /// P2P network port
    pub p2p_port: u16,
    /// Bootstrap node for P2P networking
    pub bootstrap_node: Option<String>,
    /// Game PDA address
    pub game_pda: Option<String>,
    /// Wager amount in SOL
    pub wager_amount: Option<f64>,
    /// Debug mode flag
    pub debug: bool,
    /// Log file path
    pub log_file: String,
    /// AI difficulty (1-5)
    pub ai_difficulty: Option<u8>,
    /// AI side (White or Black)
    pub ai_side: Option<PlayerColor>,
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
        }
    }
}

/// Builds the Bevy application with all plugins and configuration
/// 
/// This function configures the Bevy app with:
/// - Default plugins (window, audio, logging)
/// - Custom XFChess plugins
/// - Game configuration resource
/// - Optional AI configuration
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
        // Disable bevy_egui's auto-attachment of PrimaryEguiContext. We attach
        // it manually to the persistent camera in `setup_persistent_egui_camera`
        // so additional cameras spawned later (e.g. `xf_animate`'s mini
        // showcase camera) cannot accidentally claim the primary egui context.
        .insert_resource(bevy_egui::EguiGlobalSettings {
            auto_create_primary_context: false,
            ..default()
        })
        // Startup (not PreStartup): the primary window must exist before we spawn
        // a Camera3d so that bevy_egui can attach an egui context to it.
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

    // Add custom plugins - split into groups due to Bevy tuple size limits
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
    ))
    .add_plugins(singleplayer::SingleplayerPlugin);

    #[cfg(feature = "solana")]
    app.add_plugins(solana::SolanaPlugin);

    app
}
