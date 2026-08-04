#![cfg_attr(all(windows, debug_assertions), windows_subsystem = "console")]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

/// XFChess main entry point for decentralized chess on Solana
use clap::Parser;
use xfchess::{build_app, GameConfig, PlayerColor};

fn main() {
    // Initialize telemetry / crash reporting
    xfchess::core::crash::setup_enhanced_panic_hook();

    // Check wallet mode (Tauri vs standalone)
    let wallet_mode = std::env::var("XFCHESS_WALLET_MODE").unwrap_or_default() == "tauri";
    if wallet_mode {
        println!(" XFChess running in Tauri mode — using system-provided signing server.");
    } else {
        println!(" XFChess running in standalone mode.");
    }
    // Always print resolved config, regardless of wallet mode — two instances
    // silently pointed at different backends (a leftover BACKEND_URL/
    // SIGNING_SERVICE_URL from an earlier dev session in the same terminal)
    // used to be undetectable short of curling /health from both. This is
    // the fastest way to rule that class of bug in or out.
    #[cfg(feature = "solana")]
    println!(
        " [CONFIG] backend={} devnet_rpc={} program_id={}",
        xfchess::multiplayer::network::vps::vps_base(),
        *xfchess::multiplayer::solana::integration::state::DEVNET_RPC_URL,
        xfchess::solana::instructions::PROGRAM_ID,
    );
    #[cfg(not(feature = "solana"))]
    println!(
        " [CONFIG] backend={}",
        xfchess::multiplayer::network::vps::vps_base()
    );
    println!("   (override backend with SIGNING_SERVICE_URL or BACKEND_URL env vars)");

    // Parse Game configuration from CLI + environment variables
    let mut game_config = GameConfig::parse();

    // Load session config from file if specified
    if game_config.session_config.is_some() {
        if let Err(e) = game_config.load_session_config() {
            eprintln!(" Failed to load session config: {}", e);
            std::process::exit(1);
        }
    }

    // Handle CLI-only commands before launching game client
    if let Some(cmd) = &game_config.command {
        match cmd {
            xfchess::Commands::Tournament { action: _action } => {
                println!(" Tournament command detected - implementation pending...");
                return;
            }
            xfchess::Commands::Debug { game_id } => {
                println!(" Starting integrated debugger for game: {}", game_id);
                // logic for integrated debug view could go here
                return;
            }
        }
    }

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║          XFChess - Decentralized Chess                 ║");
    println!("║          Ephemeral Rollups on Solana                   ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();

    // Auto-assign player color if AI side is specified
    if let Some(ai_side) = game_config.ai_side {
        if game_config.player_color.is_none() {
            game_config.player_color = Some(match ai_side {
                PlayerColor::White => PlayerColor::Black,
                PlayerColor::Black => PlayerColor::White,
            });
        }
    }

    // Print game configuration if joining a game
    if let Some(ref game_id) = game_config.game_id {
        println!(" Game ID: {}", game_id);
        println!(" Player: {:?}", game_config.player_color);
        if let Some(wager) = game_config.wager_amount {
            println!(" Wager: {} SOL", wager);
        }
        println!(
            " Session: {}...",
            game_config
                .session_pubkey
                .as_ref()
                .map(|pk| pk.get(..8).unwrap_or(""))
                .unwrap_or("N/A")
        );
        println!(" RPC: {}", game_config.rpc_url);
        println!();
    }

    // Build and run the Bevy application
    let mut app = build_app(game_config);
    app.run();
}
