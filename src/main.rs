use bevy::{
    asset::AssetMetaCheck,
    audio::{AudioPlugin, Volume},
    log::LogPlugin,
    prelude::*,
};
use bevy_egui::EguiPlugin;
use clap::Parser;

mod assets;
mod cli;
mod core;
mod engine;
mod game;
mod input;
mod multiplayer;
mod persistent_camera;
mod presentation;
mod rendering;
mod singleplayer;
#[cfg(feature = "solana")]
mod solana;
mod states;
mod ui;

pub use cli::{Cli, PlayerColor as CliPlayerColor};
pub use persistent_camera::PersistentEguiCamera;
pub use xfchess::{GameConfig, PlayerColor};

#[tokio::main]
async fn main() {
    // Parse CLI arguments
    let mut cli = Cli::parse();

    // Load session config from file if specified
    if cli.session_config.is_some() {
        if let Err(e) = cli.load_session_config() {
            eprintln!("❌ Failed to load session config: {}", e);
            std::process::exit(1);
        }
    }

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║          XFChess - Decentralized Chess                 ║");
    println!("║          Ephemeral Rollups on Solana                   ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();

    // Build game config from CLI
    let game_config = GameConfig {
        game_id: cli.game_id,
        player_color: cli.player_color.map(|c| match c {
            CliPlayerColor::White => PlayerColor::White,
            CliPlayerColor::Black => PlayerColor::Black,
        }),
        rpc_url: cli.rpc_url,
        session_key: cli.session_key,
        session_pubkey: cli.session_pubkey,
        p2p_port: cli.p2p_port,
        bootstrap_node: cli.bootstrap_node,
        game_pda: cli.game_pda,
        wager_amount: cli.wager_amount,
        debug: cli.debug,
        log_file: cli.log_file.to_string_lossy().to_string(),
    };

    if game_config.game_id.is_some() {
        println!("🎮 Game ID: {}", game_config.game_id.unwrap());
        println!("🎨 Player: {:?}", game_config.player_color);
        if let Some(wager) = game_config.wager_amount {
            println!("💰 Wager: {} SOL", wager);
        }
        println!(
            "🔑 Session: {}...",
            game_config
                .session_pubkey
                .as_ref()
                .unwrap_or(&"N/A".to_string())
                .get(..8)
                .unwrap_or("")
        );
        println!("🌐 RPC: {}", game_config.rpc_url);
        println!();
    }

    let handle = tokio::runtime::Handle::current();
    let mut app = App::new();
    app.insert_resource(multiplayer::TokioRuntime(handle))
        .insert_resource(game_config)
        .init_resource::<PersistentEguiCamera>()
        .add_systems(PreStartup, persistent_camera::setup_persistent_egui_camera);

    // Add core plugins
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                // Wasm builds will check for meta files (that don't exist) if this isn't set
                meta_check: AssetMetaCheck::Never,
                // Use the project root assets folder so the game works regardless of
                // which directory the executable is launched from.
                #[cfg(not(target_arch = "wasm32"))]
                file_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .to_string_lossy()
                    .into_owned(),
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
            // Using volume from default to reduce popping sounds
            .set(AudioPlugin {
                global_volume: GlobalVolume {
                    volume: Volume::Linear(0.3),
                },
                ..default()
            })
            // Disable console logging in release mode to reduce WASM size
            .set(LogPlugin {
                filter: if cfg!(debug_assertions) {
                    "info,wgpu_core=warn,wgpu_hal=warn,xfchess=debug".to_string()
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
        states::multiplayer_menu::MultiplayerMenuPlugin,
        states::game_over::GameOverPlugin,
        states::pause::PausePlugin,
        states::piece_viewer::PieceViewerPlugin,
    ))
    .add_plugins(singleplayer::SingleplayerPlugin);

    #[cfg(feature = "solana")]
    app.add_plugins((
        solana::SolanaPlugin,
        multiplayer::rollup::mvp_plugin::EphemeralMvpPlugin,
        multiplayer::wager_state::WagerPlugin,
    ));

    app.add_plugins(multiplayer::MultiplayerPlugin);

    // Add transaction debugger if debug mode enabled
    if cli.debug {
        println!("🔍 Transaction debugger enabled");
        let log_file = std::path::PathBuf::from(&cli.log_file);
        app.add_plugins(
            multiplayer::ui::tx_debugger::TransactionDebuggerPlugin {
                log_file: Some(log_file),
                pretty_print: !cli.no_pretty_print,
                game_id: cli.game_id,
            },
        );
    }

    // Run the app
    app.run();
}
