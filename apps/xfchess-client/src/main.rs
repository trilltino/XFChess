#![cfg_attr(all(windows, debug_assertions), windows_subsystem = "console")]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use clap::Parser;
use xfchess_app::{build_app, GameConfig, PlayerColor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    xfchess_app::core::diagnostics::panic_hook::setup_panic_hook();

    // Check wallet mode (Tauri vs standalone)
    let wallet_mode = std::env::var("XFCHESS_WALLET_MODE").unwrap_or_default() == "tauri";
    if wallet_mode {
        println!(" XFChess running in Tauri mode — using system-provided signing server.");
    } else {
        println!(" XFChess running in standalone mode.");
    }

    #[cfg(feature = "solana")]
    println!(
        " [CONFIG] backend={} devnet_rpc={} program_id={}",
        xfchess_app::multiplayer::network::vps::vps_base(),
        *xfchess_app::multiplayer::solana::integration::state::DEVNET_RPC_URL,
        xfchess_app::solana::instructions::PROGRAM_ID,
    );
    #[cfg(not(feature = "solana"))]
    println!(
        " [CONFIG] backend={}",
        xfchess_app::multiplayer::network::vps::vps_base()
    );
    println!("   (override backend with SIGNING_SERVICE_URL or BACKEND_URL env vars)");

    // Parse Game configuration from CLI + environment variables
    let mut game_config = GameConfig::parse();

    if game_config.session_config.is_some() {
        if let Err(e) = game_config.load_session_config() {
            eprintln!(" Failed to load session config: {}", e);
            std::process::exit(1);
        }
    }

    // Handle CLI-only commands before launching game client
    if let Some(cmd) = &game_config.command {
        match cmd {
            xfchess_app::Commands::Tournament { action: _action } => {
                println!(" Tournament command detected - implementation pending...");
                return Ok(());
            }
            xfchess_app::Commands::Debug { game_id } => {
                println!(" Starting integrated debugger for game: {}", game_id);
                // logic for integrated debug view could go here
                return Ok(());
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

    let mut app = build_app(game_config)?;
    app.run();
    Ok(())
}
