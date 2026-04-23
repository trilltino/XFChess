/// XFChess main entry point for decentralized chess on Solana
use clap::Parser;
use std::sync::Arc;
use sqlx::SqlitePool;
use backend::signing::{AppState as SigningAppState, SigningConfig, build_router as build_signing_router};
use xfchess::{Cli, PlayerColor as CliPlayerColor, GameConfig, PlayerColor, build_app};

#[tokio::main]
async fn main() {
    // Check wallet mode (Tauri vs standalone)
    let wallet_mode = std::env::var("XFCHESS_WALLET_MODE").unwrap_or_default() == "tauri";
    if !wallet_mode {
        println!("🚀 XFChess running in standalone mode — using external HTTP backend at http://localhost:8090");
        println!("   (Embedded signing server disabled - use start_xfchess.bat to start backend)");
    } else {
        println!("🚀 XFChess running in Tauri mode — using system-provided signing server.");
    }

    // Parse CLI arguments
    let mut cli = Cli::parse();

    // Load session config from file if specified
    if cli.session_config.is_some() {
        if let Err(e) = cli.load_session_config() {
            eprintln!("❌ Failed to load session config: {}", e);
            std::process::exit(1);
        }
    }

    // Handle CLI-only commands before launching game client
    if let Some(cmd) = &cli.command {
        match cmd {
            xfchess::cli::Commands::Tournament { action } => {
                let rpc = cli.rpc_url.clone();
                let vps = "http://127.0.0.1:8090".to_string();
                let keypair = "keys/fee-payer.json";
                #[cfg(feature = "solana")]
                xfchess::cli::tournament_admin::run(action, &rpc, &vps, keypair);
                #[cfg(not(feature = "solana"))]
                eprintln!("Tournament admin tools require the 'solana' feature to be enabled during compilation!");
                return;
            }
            _ => {}
        }
    }

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║          XFChess - Decentralized Chess                 ║");
    println!("║          Ephemeral Rollups on Solana                   ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();

    // Build game config from CLI + environment variables
    let mut game_config = GameConfig {
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
        ai_difficulty: std::env::var("XFCHESS_AI_DIFFICULTY").ok().and_then(|v| v.parse().ok()),
        ai_side: std::env::var("XFCHESS_AI_SIDE").ok().map(|v| {
            if v.to_lowercase() == "white" { PlayerColor::White } else { PlayerColor::Black }
        }),
    };

    // Auto-assign player color if AI side is specified
    if game_config.player_color.is_none() && game_config.ai_side.is_some() {
        game_config.player_color = Some(match game_config.ai_side.unwrap() {
            PlayerColor::White => PlayerColor::Black,
            PlayerColor::Black => PlayerColor::White,
        });
    }

    // Print game configuration if joining a game
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
