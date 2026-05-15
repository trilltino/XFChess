#![cfg_attr(all(windows, debug_assertions), windows_subsystem = "console")]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

/// XFChess main entry point for decentralized chess on Solana
use clap::Parser;
use std::sync::Arc;
use sqlx::SqlitePool;
use backend::signing::{AppState as SigningAppState, SigningConfig, build_router as build_signing_router};
use xfchess::{GameConfig, PlayerColor, build_app};

#[tokio::main]
async fn main() {
    // Initialize telemetry / crash reporting
    xfchess::core::crash::setup_enhanced_panic_hook();

    // Check wallet mode (Tauri vs standalone)
    let wallet_mode = std::env::var("XFCHESS_WALLET_MODE").unwrap_or_default() == "tauri";
    if !wallet_mode {
        println!(" XFChess running in standalone mode — using external HTTP backend at http://localhost:8090");
        println!("   (Embedded signing server disabled - use start_xfchess.bat to start backend)");
    } else {
        println!(" XFChess running in Tauri mode — using system-provided signing server.");
    }

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

// ---------------------------------------------------------------------------
// Embedded VPS Signing Server
// ---------------------------------------------------------------------------

#[allow(dead_code)]
async fn start_embedded_signing_server() {
    dotenvy::dotenv().ok();
    
    let config = SigningConfig::from_env();
    let port = config.port;
    
    // SQLite pool for session persistence
    let pool = match SqlitePool::connect("sqlite://sessions.db?mode=rwc").await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[SIGN-SRV] Failed to connect to SQLite sessions.db: {}", e);
            return;
        }
    };
    
    // SQLite pool for vault persistence
    let vault_pool = match sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite://vault.db?mode=rwc")
        .await 
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[SIGN-SRV] Failed to connect to SQLite vault.db: {}", e);
            return;
        }
    };
    
    let tournament_store = Arc::new(backend::signing::storage::tournament::TournamentStore::new(pool.clone()).await);
    let state = SigningAppState::new(config, pool.clone(), vault_pool.clone(), tournament_store);
    if let Err(e) = state.store.init().await {
        eprintln!("[SIGN-SRV] Failed to init session store: {}", e);
        return;
    }
    
    let app = build_signing_router(state);
    let addr = format!("0.0.0.0:{}", port);
    
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[SIGN-SRV] Failed to bind to {}: {}", addr, e);
            return;
        }
    };
    
    println!("[SIGN-SRV] VPS signing server listening on http://{}", addr);
    
    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| eprintln!("[SIGN-SRV] Server error: {}", e));
}

